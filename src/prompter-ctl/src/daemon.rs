use crate::capture::{Capture, Frame};
use crate::card::Card;
use crate::{control, hypr, scripts, text};
use ab_glyph::FontVec;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::inotify;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::mem::MaybeUninit;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use wayland_client::backend::WaylandError;

const DEVICE: &str = "/dev/dri/prompter";
const OUTPUT: &str = "PROMPTER";
const EMPTY: &str = "No script loaded.\n\nWrite one in the Omgato panel, Prompter tab.";

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Off,
    Script,
    Monitor,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    mode: Mode,
    last_on: Mode,
    script: String,
    speed: u32,
    font: u32,
    mirror: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { mode: Mode::Script, last_on: Mode::Script, script: String::new(), speed: 60, font: 56, mirror: false }
    }
}

fn settings_path() -> PathBuf {
    dirs::state_dir().unwrap_or_else(|| PathBuf::from(".")).join("prompter-ctl").join("settings.json")
}

#[derive(Serialize)]
struct Status<'a> {
    running: bool,
    mode: Mode,
    script: &'a str,
    playing: bool,
    speed: u32,
    font: u32,
    mirror: bool,
    progress: f32,
}

struct Daemon {
    card: Card,
    font: FontVec,
    s: Settings,
    page: text::Page,
    offset: f32,
    playing: bool,
    tick: Instant,
    cap: Option<Capture>,
}

fn blit(frame: &Frame, dst: &mut [u8], pitch: usize, width: usize, height: usize) {
    let w = frame.width.min(width);
    let h = frame.height.min(height);
    for y in 0..h {
        let sy = if frame.y_invert { frame.height - 1 - y } else { y };
        let src = &frame.data[sy * frame.stride..sy * frame.stride + w * 4];
        let out = &mut dst[y * pitch..y * pitch + w * 4];
        if frame.bgr {
            for (o, s) in out.as_chunks_mut::<4>().0.iter_mut().zip(src.as_chunks::<4>().0) {
                *o = [s[2], s[1], s[0], s[3]];
            }
        } else {
            out.copy_from_slice(src);
        }
    }
}

impl Daemon {
    fn max_offset(&self) -> f32 {
        self.page.height.saturating_sub(self.card.height) as f32
    }

    fn relayout(&mut self, keep_place: bool) {
        let progress = if keep_place { self.offset / self.max_offset().max(1.0) } else { 0.0 };
        let text = match self.s.script.as_str() {
            "" => EMPTY.to_owned(),
            name => scripts::read(name).unwrap_or_else(|e| e),
        };
        self.page = text::layout(&self.font, &text, self.s.font as f32, self.card.width, self.card.height);
        self.offset = (progress * self.max_offset()).round();
    }

    fn repaint(&mut self) -> Result<(), String> {
        if self.s.mode != Mode::Script {
            return Ok(());
        }
        let (font, page, offset, mirror) = (&self.font, &self.page, self.offset as usize, self.s.mirror);
        self.card.paint(|dst, pitch| text::draw(font, page, offset, mirror, dst, pitch))
    }

    fn enter(&mut self) -> Result<(), String> {
        match self.s.mode {
            Mode::Off => self.card.light(false),
            Mode::Script => {
                self.card.light(true)?;
                self.repaint()
            }
            Mode::Monitor => {
                self.card.light(true)?;
                hypr::ensure_output(OUTPUT)?;
                let mut cap = Capture::connect(OUTPUT)?;
                cap.request();
                self.cap = Some(cap);
                Ok(())
            }
        }
    }

    fn leave(&mut self) -> Result<(), String> {
        self.playing = false;
        if self.cap.take().is_some() { hypr::remove_output(OUTPUT) } else { Ok(()) }
    }

    fn set_mode(&mut self, mode: Mode) -> Result<(), String> {
        if mode == self.s.mode {
            return Ok(());
        }
        self.leave()?;
        self.s.mode = mode;
        if mode != Mode::Off {
            self.s.last_on = mode;
        }
        self.enter()
    }

    fn step(&mut self, lines: f32) -> Result<(), String> {
        self.offset = (self.offset + lines * self.page.line as f32).clamp(0.0, self.max_offset());
        self.repaint()
    }

    fn command(&mut self, line: &str) -> Result<(), String> {
        let (verb, arg) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
        let num = || arg.parse::<u32>().map_err(|_| format!("{verb} needs a number, got {arg:?}"));
        match verb {
            "status" => return Ok(()),
            "on" => self.set_mode(self.s.last_on)?,
            "off" => self.set_mode(Mode::Off)?,
            "mode" => {
                let mode = <Mode as clap::ValueEnum>::from_str(arg, true).map_err(|_| format!("unknown mode {arg:?}"))?;
                self.set_mode(mode)?
            }
            "load" => {
                scripts::read(arg)?;
                self.s.script = arg.trim().to_owned();
                self.playing = false;
                self.relayout(false);
                self.set_mode(Mode::Script)?;
                self.repaint()?
            }
            "play" | "pause" | "toggle" => {
                self.set_mode(Mode::Script)?;
                self.playing = match verb { "play" => true, "pause" => false, _ => !self.playing };
                if self.playing && self.offset >= self.max_offset() {
                    self.offset = 0.0;
                }
                self.tick = Instant::now();
            }
            "top" => {
                self.offset = 0.0;
                self.repaint()?
            }
            "back" => self.step(-2.0)?,
            "forward" => self.step(2.0)?,
            "speed" => self.s.speed = num()?.clamp(5, 600),
            "faster" => self.s.speed = (self.s.speed + 10).min(600),
            "slower" => self.s.speed = self.s.speed.saturating_sub(10).max(5),
            "font" | "bigger" | "smaller" => {
                self.s.font = match verb { "bigger" => self.s.font + 4, "smaller" => self.s.font.saturating_sub(4), _ => num()? }.clamp(20, 200);
                self.relayout(true);
                self.repaint()?
            }
            "mirror" => {
                self.s.mirror = match arg { "on" => true, "off" => false, "" | "toggle" => !self.s.mirror, _ => return Err(format!("mirror on, off or toggle, got {arg:?}")) };
                self.repaint()?
            }
            _ => return Err(format!("unknown command {verb:?}")),
        }
        if let Some(dir) = settings_path().parent() {
            fs::create_dir_all(dir).ok();
        }
        fs::write(settings_path(), serde_json::to_vec_pretty(&self.s).unwrap_or_default())
            .map_err(|e| format!("save settings: {e}"))
    }

    fn status(&self) -> String {
        let progress = if self.max_offset() > 0.0 { self.offset / self.max_offset() } else { 0.0 };
        serde_json::to_string(&Status {
            running: true,
            mode: self.s.mode,
            script: &self.s.script,
            playing: self.playing,
            speed: self.s.speed,
            font: self.s.font,
            mirror: self.s.mirror,
            progress: (progress * 100.0).round() / 100.0,
        })
        .unwrap_or_default()
    }

    fn serve(&mut self, stream: UnixStream) {
        stream.set_read_timeout(Some(Duration::from_secs(1))).ok();
        let mut line = String::new();
        if BufReader::new(&stream).read_line(&mut line).is_err() {
            return;
        }
        let reply = match self.command(&line) {
            Ok(()) => self.status(),
            Err(e) => serde_json::json!({ "error": e }).to_string(),
        };
        let mut stream = stream;
        stream.write_all(format!("{reply}\n").as_bytes()).ok();
    }

    fn advance(&mut self) -> Result<(), String> {
        let now = Instant::now();
        let dt = now.duration_since(self.tick).as_secs_f32();
        self.tick = now;
        let before = self.offset as usize;
        self.offset = (self.offset + self.s.speed as f32 * dt).min(self.max_offset());
        if self.offset >= self.max_offset() {
            self.playing = false;
        }
        if self.offset as usize != before { self.repaint() } else { Ok(()) }
    }

    fn pump(&mut self) -> Result<(), String> {
        let (Some(cap), card) = (self.cap.as_mut(), &mut self.card) else { return Ok(()) };
        let (w, h) = (card.width, card.height);
        while let Some(frame) = cap.poll_frame()? {
            card.paint(|dst, pitch| blit(&frame, dst, pitch, w, h))?;
            cap.request();
        }
        Ok(())
    }
}

fn watch_scripts() -> Result<std::os::fd::OwnedFd, String> {
    fs::create_dir_all(scripts::dir()).map_err(|e| format!("create {}: {e}", scripts::dir().display()))?;
    let fd = inotify::init(inotify::CreateFlags::CLOEXEC | inotify::CreateFlags::NONBLOCK).map_err(|e| format!("inotify: {e}"))?;
    inotify::add_watch(&fd, scripts::dir(), inotify::WatchFlags::CLOSE_WRITE | inotify::WatchFlags::MOVED_TO)
        .map_err(|e| format!("watch {}: {e}", scripts::dir().display()))?;
    Ok(fd)
}

fn touched_current(fd: &std::os::fd::OwnedFd, current: &str) -> bool {
    let mut buf = [MaybeUninit::<u8>::uninit(); 4096];
    let mut reader = inotify::Reader::new(fd, &mut buf);
    let want = format!("{current}.md");
    let mut hit = false;
    while let Ok(ev) = reader.next() {
        hit |= !current.is_empty() && ev.file_name().is_some_and(|n| n.to_bytes() == want.as_bytes());
    }
    hit
}

fn listen() -> Result<UnixListener, String> {
    let path = control::socket_path()?;
    if let Some(dir) = path.parent() {
        fs::DirBuilder::new().recursive(true).mode(0o700).create(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    fs::remove_file(&path).ok();
    let listener = UnixListener::bind(&path).map_err(|e| format!("bind {}: {e}", path.display()))?;
    listener.set_nonblocking(true).map_err(|e| format!("listener: {e}"))?;
    Ok(listener)
}

pub fn run() -> Result<(), String> {
    let (stop, stop_tx) = UnixStream::pair().map_err(|e| format!("signal pipe: {e}"))?;
    for sig in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        let tx = stop_tx.try_clone().map_err(|e| format!("signal pipe: {e}"))?;
        signal_hook::low_level::pipe::register(sig, tx).map_err(|e| format!("signal: {e}"))?;
    }
    let card = Card::open(DEVICE)?;
    let font = text::load_font()?;
    let s: Settings = fs::read(settings_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let page = text::layout(&font, "", s.font as f32, card.width, card.height);
    let mut d = Daemon { card, font, s, page, offset: 0.0, playing: false, tick: Instant::now(), cap: None };
    d.relayout(false);
    let watch = watch_scripts()?;
    let listener = listen()?;
    let result = d.enter().and_then(|()| serve(&mut d, &stop, &listener, &watch));
    let left = d.leave();
    if let Ok(path) = control::socket_path() {
        fs::remove_file(path).ok();
    }
    result.and(left)
}

fn serve(d: &mut Daemon, stop: &UnixStream, listener: &UnixListener, watch: &std::os::fd::OwnedFd) -> Result<(), String> {
    let frame = Timespec { tv_sec: 0, tv_nsec: 16_666_667 };
    loop {
        d.pump()?;
        let guard = match d.cap.as_mut() {
            Some(cap) => match cap.prepare_read()? {
                Some(g) => Some(g),
                None => continue,
            },
            None => None,
        };
        let ready = {
            let conn = guard.as_ref().map(|g| g.connection_fd());
            let mut fds = vec![
                PollFd::new(stop, PollFlags::IN),
                PollFd::new(listener, PollFlags::IN),
                PollFd::new(watch, PollFlags::IN),
            ];
            if let Some(c) = conn.as_ref() {
                fds.push(PollFd::new(c, PollFlags::IN));
            }
            match poll(&mut fds, d.playing.then_some(&frame)) {
                Ok(_) => {}
                Err(rustix::io::Errno::INTR) => continue,
                Err(e) => return Err(format!("poll: {e}")),
            }
            fds.iter().map(|f| !f.revents().is_empty()).collect::<Vec<_>>()
        };
        if ready[0] {
            return Ok(());
        }
        if let Some(g) = guard
            && ready.get(3).copied().unwrap_or(false)
        {
            match g.read() {
                Ok(_) => {}
                Err(WaylandError::Io(e)) if e.kind() == ErrorKind::WouldBlock => {}
                Err(e) => return Err(format!("wayland read: {e}")),
            }
        }
        if ready[1] {
            while let Ok((stream, _)) = listener.accept() {
                stream.set_nonblocking(false).ok();
                d.serve(stream);
            }
        }
        if ready[2] && touched_current(watch, &d.s.script) {
            d.relayout(true);
            d.repaint()?;
        }
        if d.playing {
            d.advance()?;
        }
    }
}
