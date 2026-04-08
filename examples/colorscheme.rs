//! Generates an HTML colorscheme preview.
//! Run with: cargo run --example colorscheme > colorscheme.html && xdg-open colorscheme.html

fn main() {
    let terrains = [
        ("Empty", "#323232"),
        ("House", "#FF7A2F"),
        ("Forest", "#594024"),
        ("Wheat", "#FFF226"),
        ("Rail", "#B0B5BD"),
        ("River", "#1A80E6"),
        ("Lake", "#1A56C9"),
        ("Station", "#B0B5BD"),
    ];

    println!(
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Colorscheme</title>
<style>
  body {{ background: #1a1a1a; color: #eee; font-family: sans-serif; padding: 20px; }}
  .swatches {{ display: flex; gap: 16px; flex-wrap: wrap; margin: 20px 0; }}
  .swatch {{ text-align: center; }}
  .swatch svg {{ display: block; margin: 0 auto 4px; }}
  .swatch .name {{ font-size: 13px; }}
  .swatch .hex {{ font-size: 11px; color: #999; }}
  h2 {{ margin-top: 30px; }}
  .tiles {{ display: flex; gap: 20px; flex-wrap: wrap; margin: 20px 0; }}
  .tile {{ text-align: center; }}
  .tile .label {{ font-size: 11px; margin-top: 4px; }}
  table {{ border-collapse: collapse; }}
  td {{ padding: 2px; }}
  th {{ font-size: 10px; padding: 4px; }}
</style></head><body>
<h1>Terrain Colorscheme</h1>"#
    );

    // Individual swatches.
    println!(r#"<div class="swatches">"#);
    for (name, hex) in &terrains {
        println!(
            r#"<div class="swatch">
  <svg width="80" height="80" viewBox="-1 -1 2 2">{}</svg>
  <div class="name">{name}</div>
  <div class="hex">{hex}</div>
</div>"#,
            svg_hex(hex, 0.9)
        );
    }
    println!("</div>");

    // Example tiles.
    println!("<h2>Example Tiles</h2>");
    println!(r#"<div class="tiles">"#);

    // Forest 3 + House 2 + Wheat 1
    print_tile(
        "3F+2H+1W",
        &[
            (&[0, 1, 2], terrains[2].1),
            (&[3, 4], terrains[1].1),
            (&[5], terrains[3].1),
        ],
    );
    // Straight river through wheat/forest
    print_tile(
        "River straight",
        &[
            (&[0, 3], terrains[5].1),
            (&[1, 2], terrains[3].1),
            (&[4, 5], terrains[2].1),
        ],
    );
    // Rail bridge
    print_tile(
        "Rail bridge",
        &[
            (&[0, 2], terrains[4].1),
            (&[1], terrains[2].1),
            (&[3, 4, 5], terrains[1].1),
        ],
    );
    // Station
    print_tile("Station", &[(&[0, 1, 2, 3, 4, 5], terrains[7].1)]);
    // Lake
    print_tile("Lake", &[(&[0, 1, 2, 3, 4, 5], terrains[6].1)]);

    println!("</div>");

    // Adjacency matrix.
    println!("<h2>Adjacency Preview</h2>");
    println!("<table><tr><th></th>");
    for (name, _) in &terrains {
        println!("<th>{name}</th>");
    }
    println!("</tr>");
    for (name_a, hex_a) in &terrains {
        println!("<tr><th>{name_a}</th>");
        for (_, hex_b) in &terrains {
            println!(r#"<td><svg width="50" height="50" viewBox="-1 -1 2 2">"#);
            // Left half = A, right half = B.
            for side in 0..3 {
                println!("{}", svg_wedge(hex_a, side));
            }
            for side in 3..6 {
                println!("{}", svg_wedge(hex_b, side));
            }
            println!(r#"{}</svg></td>"#, svg_hex_outline());
        }
        println!("</tr>");
    }
    println!("</table>");

    println!("</body></html>");
}

fn hex_point(side: usize, r: f64) -> (f64, f64) {
    let angle = std::f64::consts::FRAC_PI_3 * side as f64 - std::f64::consts::FRAC_PI_2;
    (r * angle.cos(), r * angle.sin())
}

fn svg_hex(fill: &str, r: f64) -> String {
    let points: String = (0..6)
        .map(|i| {
            let (x, y) = hex_point(i, r);
            format!("{x:.3},{y:.3}")
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!(r#"<polygon points="{points}" fill="{fill}" stroke="gray" stroke-width="0.04"/>"#)
}

fn svg_hex_outline() -> String {
    let points: String = (0..6)
        .map(|i| {
            let (x, y) = hex_point(i, 0.9);
            format!("{x:.3},{y:.3}")
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!(r#"<polygon points="{points}" fill="none" stroke="gray" stroke-width="0.04"/>"#)
}

fn svg_wedge(fill: &str, side: usize) -> String {
    let (x0, y0) = hex_point((side + 5) % 6, 0.9);
    let (x1, y1) = hex_point(side, 0.9);
    format!(
        r#"<polygon points="0,0 {x0:.3},{y0:.3} {x1:.3},{y1:.3}" fill="{fill}" stroke="none"/>"#
    )
}

fn print_tile(label: &str, segments: &[(&[usize], &str)]) {
    println!(r#"<div class="tile"><svg width="100" height="100" viewBox="-1 -1 2 2">"#);

    let mut side_color = ["#323232"; 6];
    let mut side_seg = [usize::MAX; 6];
    for (seg_idx, &(sides, color)) in segments.iter().enumerate() {
        for &s in sides {
            side_color[s] = color;
            side_seg[s] = seg_idx;
        }
    }

    for (side, &color) in side_color.iter().enumerate() {
        println!("{}", svg_wedge(color, side));
    }

    // Segment borders.
    for side in 0..6 {
        let next = (side + 1) % 6;
        if side_seg[side] != side_seg[next] {
            let (x, y) = hex_point(side, 0.9);
            println!(
                r#"<line x1="0" y1="0" x2="{x:.3}" y2="{y:.3}" stroke="black" stroke-width="0.05"/>"#
            );
        }
    }

    println!("{}", svg_hex_outline());
    println!(r#"</svg><div class="label">{label}</div></div>"#);
}
