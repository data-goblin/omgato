use rustix::fs::{MemfdFlags, ftruncate, memfd_create};
use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};
use std::ffi::c_void;
use std::os::fd::{AsFd, OwnedFd};
use wayland_client::backend::ReadEventsGuard;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_output::{self, WlOutput};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_shm::{Format, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, WEnum, delegate_noop};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::{self, Flags, ZwlrScreencopyFrameV1};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

pub struct Frame<'a> {
    pub data: &'a [u8],
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub bgr: bool,
    pub y_invert: bool,
}

#[derive(Clone, Copy, PartialEq)]
struct Spec {
    format: Format,
    width: u32,
    height: u32,
    stride: u32,
}

#[derive(Default)]
struct State {
    outputs: Vec<(WlOutput, String)>,
    spec: Option<Spec>,
    buffer_done: bool,
    y_invert: bool,
    ready: bool,
    failed: bool,
}

struct Shm {
    _fd: OwnedFd,
    ptr: *mut c_void,
    len: usize,
    pool: WlShmPool,
    buffer: WlBuffer,
    spec: Spec,
}

impl Drop for Shm {
    fn drop(&mut self) {
        self.buffer.destroy();
        self.pool.destroy();
        unsafe { munmap(self.ptr, self.len).ok() };
    }
}

pub struct Capture {
    _conn: Connection,
    queue: EventQueue<State>,
    qh: QueueHandle<State>,
    state: State,
    wl_shm: WlShm,
    manager: ZwlrScreencopyManagerV1,
    output: WlOutput,
    shm: Option<Shm>,
    frame: Option<ZwlrScreencopyFrameV1>,
    copying: bool,
}

fn bgr(format: Format) -> Result<bool, String> {
    match format {
        Format::Xrgb8888 | Format::Argb8888 => Ok(false),
        Format::Xbgr8888 | Format::Abgr8888 => Ok(true),
        other => Err(format!("unsupported screencopy format {other:?}")),
    }
}

impl Capture {
    pub fn connect(output_name: &str) -> Result<Self, String> {
        let conn = Connection::connect_to_env().map_err(|e| format!("wayland: {e}"))?;
        let (globals, mut queue) = registry_queue_init::<State>(&conn).map_err(|e| format!("registry: {e}"))?;
        let qh = queue.handle();
        let wl_shm: WlShm = globals.bind(&qh, 1..=1, ()).map_err(|e| format!("wl_shm: {e}"))?;
        let manager: ZwlrScreencopyManagerV1 = globals.bind(&qh, 3..=3, ()).map_err(|e| format!("screencopy: {e}"))?;
        let mut state = State::default();
        for g in globals.contents().clone_list().into_iter().filter(|g| g.interface == "wl_output") {
            if g.version >= 4 {
                let o: WlOutput = globals.registry().bind(g.name, 4, &qh, ());
                state.outputs.push((o, String::new()));
            }
        }
        queue.roundtrip(&mut state).map_err(|e| format!("roundtrip: {e}"))?;
        let output = state.outputs.iter().find(|(_, n)| n == output_name).map(|(o, _)| o.clone())
            .ok_or_else(|| format!("wayland output {output_name} not found"))?;
        Ok(Capture { _conn: conn, queue, qh, state, wl_shm, manager, output, shm: None, frame: None, copying: false })
    }

    pub fn request(&mut self) {
        if let Some(f) = self.frame.take() {
            f.destroy();
        }
        self.state.spec = None;
        self.state.buffer_done = false;
        self.state.y_invert = false;
        self.state.ready = false;
        self.state.failed = false;
        self.copying = false;
        self.frame = Some(self.manager.capture_output(0, &self.output, &self.qh, ()));
    }

    fn ensure_shm(&mut self, spec: Spec) -> Result<(), String> {
        if self.shm.as_ref().is_some_and(|s| s.spec == spec) {
            return Ok(());
        }
        self.shm = None;
        let len = (spec.stride * spec.height) as usize;
        let fd = memfd_create("prompter-ctl", MemfdFlags::CLOEXEC).map_err(|e| format!("memfd: {e}"))?;
        ftruncate(&fd, len as u64).map_err(|e| format!("ftruncate: {e}"))?;
        let ptr = unsafe { mmap(std::ptr::null_mut(), len, ProtFlags::READ | ProtFlags::WRITE, MapFlags::SHARED, &fd, 0) }
            .map_err(|e| format!("mmap: {e}"))?;
        let pool = self.wl_shm.create_pool(fd.as_fd(), len as i32, &self.qh, ());
        let buffer = pool.create_buffer(0, spec.width as i32, spec.height as i32, spec.stride as i32, spec.format, &self.qh, ());
        self.shm = Some(Shm { _fd: fd, ptr, len, pool, buffer, spec });
        Ok(())
    }

    pub fn poll_frame(&mut self) -> Result<Option<Frame<'_>>, String> {
        self.queue.dispatch_pending(&mut self.state).map_err(|e| format!("dispatch: {e}"))?;
        if self.state.failed {
            return Err("screencopy failed; the output is gone".into());
        }
        if self.state.buffer_done && !self.copying {
            let spec = self.state.spec.ok_or("compositor offered no shm buffer")?;
            bgr(spec.format)?;
            self.ensure_shm(spec)?;
            if let (Some(frame), Some(shm)) = (&self.frame, &self.shm) {
                frame.copy_with_damage(&shm.buffer);
            }
            self.copying = true;
        }
        if !self.state.ready {
            return Ok(None);
        }
        let shm = self.shm.as_ref().ok_or("ready without a buffer")?;
        Ok(Some(Frame {
            data: unsafe { std::slice::from_raw_parts(shm.ptr as *const u8, shm.len) },
            width: shm.spec.width as usize,
            height: shm.spec.height as usize,
            stride: shm.spec.stride as usize,
            bgr: bgr(shm.spec.format)?,
            y_invert: self.state.y_invert,
        }))
    }

    pub fn prepare_read(&mut self) -> Result<Option<ReadEventsGuard>, String> {
        self.queue.flush().map_err(|e| format!("flush: {e}"))?;
        Ok(self.queue.prepare_read())
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(_: &mut Self, _: &WlRegistry, _: <WlRegistry as wayland_client::Proxy>::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<WlOutput, ()> for State {
    fn event(state: &mut Self, proxy: &WlOutput, event: wl_output::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let wl_output::Event::Name { name } = event
            && let Some(entry) = state.outputs.iter_mut().find(|(o, _)| o == proxy)
        {
            entry.1 = name;
        }
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, ()> for State {
    fn event(state: &mut Self, _: &ZwlrScreencopyFrameV1, event: zwlr_screencopy_frame_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer { format: WEnum::Value(format), width, height, stride } => {
                state.spec = Some(Spec { format, width, height, stride });
            }
            zwlr_screencopy_frame_v1::Event::BufferDone => state.buffer_done = true,
            zwlr_screencopy_frame_v1::Event::Flags { flags: WEnum::Value(f) } => state.y_invert = f.contains(Flags::YInvert),
            zwlr_screencopy_frame_v1::Event::Ready { .. } => state.ready = true,
            zwlr_screencopy_frame_v1::Event::Failed => state.failed = true,
            _ => {}
        }
    }
}

delegate_noop!(State: ignore WlShm);
delegate_noop!(State: ignore WlShmPool);
delegate_noop!(State: ignore WlBuffer);
delegate_noop!(State: ZwlrScreencopyManagerV1);
