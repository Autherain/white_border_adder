#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::super::{Out, Ratios};

use super::*;

fn options() -> Options {
    Options {
        width: 1080,
        height: 1080,
        landscape: Ratios {
            vert: 0.05,
            horiz: 0.03,
        },
        portrait: Ratios {
            vert: 0.005,
            horiz: 0.18,
        },
        jpeg_quality: 100,
        prefix: "bordered_".to_owned(),
        out: Out::default(),
        recursive: false,
        force: false,
    }
}

#[test]
fn layout_fits_and_centres() {
    let cases = [
        // available 1015.2 x 972, scale 0.2538 -> 1015 x 761
        (
            "landscape",
            4000,
            3000,
            Rect {
                x: 32,
                y: 159,
                width: 1015,
                height: 761,
            },
        ),
        // available 691.2 x 1069.2, scale 0.2304 -> 691 x 921
        (
            "portrait",
            3000,
            4000,
            Rect {
                x: 194,
                y: 79,
                width: 691,
                height: 921,
            },
        ),
        // square uses portrait ratios, scale 0.6912 -> 691 x 691
        (
            "square",
            1000,
            1000,
            Rect {
                x: 194,
                y: 194,
                width: 691,
                height: 691,
            },
        ),
    ];
    for (name, w, h, want) in cases {
        assert_eq!(layout(w, h, &options()), want, "{name}");
    }
}
