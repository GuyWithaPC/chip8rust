use log::{debug,error};
use pixels::{Error, Pixels, SurfaceTexture};
use winit::{
    dpi::LogicalSize,
    event::{Event, VirtualKeyCode},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;

const SCR_WIDTH: usize = 64;
const SCR_HEIGHT: usize = 32;

struct RAM {
    range: u16,
    space: Vec<u8>
}
impl RAM {
    fn empty() -> RAM {
        RAM {
            range: 4096u16,
            space: vec![0; 4096]
        }
    }
}

struct Stack {
    pointers: Vec<u16>,
    size: usize
}
impl Stack {
    fn empty() -> Stack {
        Stack {
            pointers: Vec::new(),
            size: 0
        }
    }
    fn push() {

    }
}

struct Display {
    pixels: Vec<bool> // full on all the pixels
}
impl Display {
    fn empty() -> Display {
        Display {
            pixels: vec![false;SCR_WIDTH * SCR_HEIGHT]
        }
    }
    fn set_pixel(&mut self, x: usize, y: usize, case: bool) {
        self.pixels[y*SCR_WIDTH+x] = case;
    }
    fn draw(&self, screen: &mut [u8]) {
        for (b,pix) in self.pixels.iter().zip(screen.chunks_exact_mut(4)) {
            let color = if *b {[0xff,0xff,0xff,0xff]} else {[0x00,0x00,0x00,0xff]};
            pix.copy_from_slice(&color);
        }
    }
}

fn main() -> Result<(),Error>{

    // display and event loop setup

    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();

    let window = {
        let size = LogicalSize::new(SCR_WIDTH as f64, SCR_HEIGHT as f64);
        let scaled_size = LogicalSize::new(SCR_WIDTH as f64 * 3.0, SCR_HEIGHT as f64 * 3.0);
        WindowBuilder::new()
            .with_title("chip-8 interpreter")
            .with_inner_size(scaled_size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width,window_size.height,&window);
        Pixels::new(SCR_WIDTH as u32, SCR_HEIGHT as u32, surface_texture)?
    };

    let mut display = Display::empty();
    display.set_pixel(0,0,true);

    // display setup finished. now processor setup begins.



    // processor setup finished. event loop now.

    event_loop.run(move |event, _, control_flow| {

        if let Event::RedrawRequested(_) = event {
            display.draw(pixels.get_frame_mut());
            if pixels.render()
                .map_err(|e| error!("pixels.render() failed: {}", e))
                .is_err()
            {
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        if input.update(&event) {
            if let Some(size) = input.window_resized() {
                pixels.resize_surface(size.width, size.height);
            }
            window.request_redraw();
        }
    });
}
