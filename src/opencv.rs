use opencv::{
    core::{Mat, Point2f, Point2i, Scalar, Size, Vector, CV_8UC1},
    imgcodecs::imwrite,
    imgproc::{fill_poly, LINE_8},
};

use crate::map::Map;

// A hex side is 10 px long.
const SIDE_TO_PX: f32 = 10.0;

fn draw_rect(img: &mut Mat, pos: Point2f) {
    let cos30 = std::f32::consts::FRAC_PI_6.cos();

    let to_img_coords = |fpoint: Point2f| (fpoint * SIDE_TO_PX).to::<i32>().unwrap();
    let points = Vector::from_slice(&[
        to_img_coords(Point2f::new(1.0, 0.0)),
        to_img_coords(Point2f::new(0.5, -cos30)),
        to_img_coords(Point2f::new(-0.5, -cos30)),
        to_img_coords(Point2f::new(-1.0, 0.0)),
        to_img_coords(Point2f::new(-0.5, cos30)),
        to_img_coords(Point2f::new(0.5, cos30)),
    ]);

    let black = Scalar::new(0.0, 0.0, 0.0, 0.0);
    let line_type = LINE_8;
    let shift = 0;

    // https://docs.rs/opencv/latest/opencv/imgproc/fn.fill_poly.html
    fill_poly(img, &points, black, line_type, shift, to_img_coords(pos))
        .expect("Failed to draw polygon");
}

fn map_to_img(map: &Map) -> Mat {
    let num_tiles_w = 100;
    let hex_w = 1.5;
    let width = (num_tiles_w as f32 * hex_w * SIDE_TO_PX) as i32;

    let num_tiles_h = 100;
    let hex_h = 2.0 * std::f32::consts::FRAC_PI_6.cos();
    let height = (num_tiles_h as f32 * hex_h * SIDE_TO_PX) as i32;

    let map_size = Size { width, height };
    let white = Scalar::new(255.0, 0.0, 0.0, 0.0);
    let mut full_bw_map = Mat::new_size_with_default(map_size, CV_8UC1, white)
        .expect("Failed to create BW map image");

    draw_rect(&mut full_bw_map, Point2f::new(5.0, 50.0));
    draw_rect(&mut full_bw_map, Point2f::new(10.0, 50.0));
    draw_rect(&mut full_bw_map, Point2f::new(15.0, 50.0));
    draw_rect(&mut full_bw_map, Point2f::new(20.0, 50.0));
    draw_rect(&mut full_bw_map, Point2f::new(25.0, 50.0));

    imwrite("test.png", &full_bw_map, &Vector::default()).expect("Failed to save image");
}
