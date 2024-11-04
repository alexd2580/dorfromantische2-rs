use glam::IVec2;
use std::ptr;
use std::time::Instant;
use std::{thread, time::Duration};
use x11::xlib;
use x11::xtest;

// const ALLPLANES: u64 = 0xFFFFFFFFFFFFFFFF;

#[derive(Debug, Clone)]
pub struct Screen {
    pub display: *mut xlib::_XDisplay,
    pub root_window: u64,
    pub screen_size: IVec2,
}

impl Screen {
    pub fn new() -> Self {
        unsafe {
            let display: *mut xlib::_XDisplay = xlib::XOpenDisplay(ptr::null());
            assert!(!display.is_null(), "Unable to open X display");

            let screen = xlib::XDefaultScreen(display);
            let root_window = xlib::XRootWindow(display, screen);

            let width = xlib::XDisplayWidth(display, screen);
            let height = xlib::XDisplayHeight(display, screen);

            Screen {
                display,
                root_window,
                screen_size: IVec2::new(width, height),
            }
        }
    }

    // /// returns screen dimensions. All monitors included
    // pub fn dimension(&self) -> (i32, i32) {
    //     let dimensions = (self.screen_width, self.screen_height);
    //     dimensions
    // }
    //
    // /// return region dimension which is set up when template is precalculated
    // pub fn region_dimension(&self) -> (u32, u32) {
    //     let dimensions = (self.screen_region_width, self.screen_region_height);
    //     dimensions
    // }

    // /// executes convert_bitmap_to_rgba, meaning it converts Vector of values to RGBA and crops the image
    // /// as inputted region area. Not used anywhere at the moment
    // pub fn grab_screen_image(&mut self,  region: (u32, u32, u32, u32)) -> ImageBuffer<Rgba<u8>, Vec<u8>>{
    //     let (x, y, width, height) = region;
    //     self.screen_region_width = width;
    //     self.screen_region_height = height;
    //     self.capture_screen();
    //     let image = self.convert_bitmap_to_rgba();
    //     let cropped_image: ImageBuffer<Rgba<u8>, Vec<u8>> = imgtools::cut_screen_region(x, y, width, height, &image);
    //     cropped_image
    // }
    //
    //
    // /// executes convert_bitmap_to_grayscale, meaning it converts Vector of values to grayscale and crops the image
    // /// as inputted region area
    // pub fn grab_screen_image_grayscale(&mut self,  region: &(u32, u32, u32, u32)) -> ImageBuffer<Luma<u8>, Vec<u8>>{
    //     let (x, y, width, height) = region;
    //     self.screen_region_width = *width;
    //     self.screen_region_height = *height;
    //     self.capture_screen();
    //     let image: ImageBuffer<Luma<u8>, Vec<u8>> = self.convert_bitmap_to_grayscale();
    //     let cropped_image: ImageBuffer<Luma<u8>, Vec<u8>> = imgtools::cut_screen_region(*x, *y, *width, *height, &image);
    //     cropped_image
    // }
    //
    // /// captures and saves screenshot of monitors
    // pub fn grab_screenshot(&mut self, image_path: &str) {
    //     self.capture_screen();
    //     let image = self.convert_bitmap_to_rgba();
    //     image.save(image_path).unwrap();
    // }
    //
    //
    //
    // /// first order capture screen function. it captures screen image and stores it as vector in self.pixel_data
    // fn capture_screen(&mut self) {
    //     unsafe{
    //         let ximage = XGetImage(self.display, self.root_window, 0, 0, self.screen_width as u32, self.screen_height as u32, ALLPLANES, ZPixmap);
    //         if ximage.is_null() {
    //             panic!("Unable to get X image");
    //         }
    //
    //         // get the image data
    //         let data = (*ximage).data as *mut u8;
    //         let data_len = ((*ximage).width * (*ximage).height * ((*ximage).bits_per_pixel / 8)) as usize;
    //         let slice = std::slice::from_raw_parts(data, data_len);
    //         // create an image buffer from the captured data
    //         let mut img = ImageBuffer::<Rgba<u8>, Vec<u8>>::new((*ximage).width as u32, (*ximage).height as u32);
    //         let (image_width, image_height) = img.dimensions();
    //         let mut pixel_data: Vec<u8> = Vec::with_capacity((image_width * image_height * 4) as usize);
    //
    //         for (x, y, _pixel) in img.enumerate_pixels_mut() {
    //             let index = ((y * image_width + x) * 4) as usize;
    //             pixel_data.push(slice[index + 2]); // R
    //             pixel_data.push(slice[index + 1]); // G
    //             pixel_data.push(slice[index]);     // B
    //             pixel_data.push(255);              // A
    //         }
    //         self.pixel_data = pixel_data;
    //         XDestroyImage(ximage);
    //
    //     }
    // }
    //
    // /// convert vector to Luma Imagebuffer
    // fn convert_bitmap_to_grayscale(&self) -> ImageBuffer<Luma<u8>, Vec<u8>> {
    //     let mut grayscale_data = Vec::with_capacity((self.screen_width * self.screen_height) as usize);
    //     for chunk in self.pixel_data.chunks_exact(4) {
    //         let r = chunk[2] as u32;
    //         let g = chunk[1] as u32;
    //         let b = chunk[0] as u32;
    //         // calculate the grayscale value using the luminance formula
    //         let gray_value = ((r * 30 + g * 59 + b * 11) / 100) as u8;
    //         grayscale_data.push(gray_value);
    //     }
    //     GrayImage::from_raw(
    //                 self.screen_width as u32,
    //                 self.screen_height as u32,
    //                 grayscale_data
    //                 ).expect("Couldn't convert to GrayImage")
    // }
    //
    // /// convert vector to RGBA ImageBuffer
    // fn convert_bitmap_to_rgba(&self) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    //     ImageBuffer::from_raw(
    //         self.screen_width as u32,
    //         self.screen_height as u32,
    //         self.pixel_data.clone(),
    //     ).expect("Couldn't convert to ImageBuffer")
    // }

    #[allow(clippy::unused_self)]
    pub fn sleep_ms(&self, ms: u64) {
        thread::sleep(Duration::from_millis(ms));
    }

    pub fn get_mouse_position(&self) -> IVec2 {
        unsafe {
            let mut root_return = 0;
            let mut child_return = 0;
            let mut root_x = 0;
            let mut root_y = 0;
            let mut win_x = 0;
            let mut win_y = 0;
            let mut mask_return = 0;

            let status = xlib::XQueryPointer(
                self.display,
                self.root_window,
                &mut root_return,
                &mut child_return,
                &mut root_x,
                &mut root_y,
                &mut win_x,
                &mut win_y,
                &mut mask_return,
            );

            assert!(status != 0, "Unable to query pointer position");

            IVec2::new(root_x, root_y)
        }
    }

    fn get_window_under_cursor(&self) -> Option<xlib::Window> {
        let mut child: xlib::Window = 0;
        let mut win_x: i32 = 0;
        let mut win_y: i32 = 0;

        unsafe {
            let pos = self.get_mouse_position();
            let status = xlib::XTranslateCoordinates(
                self.display,
                xlib::XDefaultRootWindow(self.display),
                xlib::XDefaultRootWindow(self.display),
                pos.x,
                pos.y,
                &mut win_x,
                &mut win_y,
                &mut child,
            );
            if status != 0 && child != 0 {
                Some(child)
            } else {
                None
            }
        }
    }

    fn focus_window(&self, window: xlib::Window) {
        unsafe {
            xlib::XSetInputFocus(
                self.display,
                window,
                xlib::RevertToParent,
                xlib::CurrentTime,
            );
            xlib::XFlush(self.display);
            self.sleep_ms(50);
        }
    }

    pub fn warp_mouse(&self, dst: IVec2) {
        unsafe {
            xlib::XWarpPointer(self.display, 0, self.root_window, 0, 0, 0, 0, dst.x, dst.y);
            xlib::XFlush(self.display);
        }
    }

    pub fn move_mouse(&self, dst: IVec2, duration: f32) {
        let start = Instant::now();
        let start_pos = self.get_mouse_position();
        let distance = dst - start_pos;

        loop {
            let percent = start.elapsed().as_secs_f32() / duration;
            let step = start_pos + distance * (percent * 100.0).min(100.0) as i32 / 100;
            self.warp_mouse(step);

            if percent > 1.0 {
                break;
            }
        }
    }

    unsafe fn test_query_extension(&self) -> bool {
        let mut event_base = 0;
        let mut error_base = 0;
        let test = xtest::XTestQueryExtension(
            self.display,
            // TODO
            &mut event_base,
            &mut error_base,
            &mut event_base,
            &mut error_base,
        );
        test != 0
    }

    fn mouse_button(&self, button: u32, pressed: bool) {
        unsafe {
            if !self.test_query_extension() {
                eprintln!("XTest extension not available");
                return;
            }
            if let Some(window) = self.get_window_under_cursor() {
                self.focus_window(window);
            }

            let pressed_state = i32::from(pressed);
            xtest::XTestFakeButtonEvent(self.display, button, pressed_state, xlib::CurrentTime);
            xlib::XFlush(self.display);
        }
    }

    /// click mouse, either left, right or middle
    pub fn mouse_click(&self, button: u32) {
        self.mouse_button(button, true);
        self.mouse_button(button, false);
    }

    pub fn mouse_drag(&self, button: u32, dst: IVec2, duration: f32) {
        self.mouse_button(button, true);
        self.move_mouse(dst, duration);
        self.mouse_button(button, false);
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        unsafe {
            xlib::XCloseDisplay(self.display);
        }
    }
}

fn navigate() {
    // let mut screenshot_dir = dirs::cache_dir().unwrap();
    // screenshot_dir.push("dorfautomatik");
    // let _ = std::fs::create_dir_all(screenshot_dir);

    let screen = Screen::new();
    screen.warp_mouse(IVec2::new(2560 * 3 / 2, 1440 / 2));

    for _ in 0..30 {
        screen.mouse_click(4);
    }

    screen.sleep_ms(2000);

    for _ in 0..30 {
        screen.mouse_click(5);
    }

    screen.sleep_ms(2000);

    screen.warp_mouse(IVec2::new(2560 + 1, 1440 / 2));
    screen.mouse_drag(1, IVec2::new(2 * 2560 - 1, 1440 / 2), 5.0);
}
