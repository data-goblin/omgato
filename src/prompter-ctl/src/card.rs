use drm::Device as _;
use drm::buffer::{Buffer, DrmFourcc};
use drm::control::{
    ClipRect, Device as ControlDevice, Mode, ModeTypeFlags, connector, crtc,
    dumbbuffer::DumbBuffer, framebuffer,
};
use std::fs::{File, OpenOptions};
use std::os::fd::{AsFd, BorrowedFd};

struct Fd(File);

impl AsFd for Fd {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl drm::Device for Fd {}
impl ControlDevice for Fd {}

pub struct Card {
    fd: Fd,
    crtc: crtc::Handle,
    conn: connector::Handle,
    mode: Mode,
    fb: framebuffer::Handle,
    buf: DumbBuffer,
    pub width: usize,
    pub height: usize,
}

fn err(what: &'static str) -> impl Fn(std::io::Error) -> String {
    move |e| format!("{what}: {e}")
}

impl Card {
    pub fn open(path: &str) -> Result<Self, String> {
        let file = OpenOptions::new().read(true).write(true).open(path)
            .map_err(|e| format!("open {path}: {e}"))?;
        let fd = Fd(file);
        fd.acquire_master_lock().map_err(|e| format!("{path} is not ours to drive (DRM master held elsewhere): {e}"))?;
        let res = fd.resource_handles().map_err(err("resources"))?;
        let conn = res.connectors().iter()
            .filter_map(|&h| fd.get_connector(h, false).ok())
            .find(|c| c.state() == connector::State::Connected)
            .ok_or("no connected connector")?;
        let mode = *conn.modes().iter()
            .find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
            .or(conn.modes().first())
            .ok_or("connector has no modes")?;
        let crtc = conn.encoders().iter()
            .filter_map(|&e| fd.get_encoder(e).ok())
            .flat_map(|e| res.filter_crtcs(e.possible_crtcs()))
            .next()
            .ok_or("no usable crtc")?;
        let (w, h) = mode.size();
        let buf = fd.create_dumb_buffer((w.into(), h.into()), DrmFourcc::Xrgb8888, 32).map_err(err("dumb buffer"))?;
        let fb = fd.add_framebuffer(&buf, 24, 32).map_err(err("framebuffer"))?;
        let mut card = Card { fd, crtc, conn: conn.handle(), mode, fb, buf, width: w.into(), height: h.into() };
        card.light(true)?;
        Ok(card)
    }

    pub fn light(&mut self, on: bool) -> Result<(), String> {
        if !on {
            return self.fd.set_crtc(self.crtc, None, (0, 0), &[], None).map_err(err("blank"));
        }
        self.fd.set_crtc(self.crtc, Some(self.fb), (0, 0), &[self.conn], Some(self.mode)).map_err(err("modeset"))?;
        self.paint(|px, _| px.fill(0))
    }

    pub fn paint(&mut self, draw: impl FnOnce(&mut [u8], usize)) -> Result<(), String> {
        let pitch = self.buf.pitch() as usize;
        let mut map = self.fd.map_dumb_buffer(&mut self.buf).map_err(err("map"))?;
        draw(&mut map, pitch);
        drop(map);
        let clip = ClipRect::new(0, 0, self.width as u16, self.height as u16);
        self.fd.dirty_framebuffer(self.fb, &[clip]).map_err(err("dirtyfb"))
    }
}
