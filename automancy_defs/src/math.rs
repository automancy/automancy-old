#![allow(unused_qualifications)]

use std::f64::consts::PI;
use std::ops::{Div, Sub};

use cgmath::{point2, point3, vec2, vec3, Angle, BaseFloat, EuclideanSpace};
use hexagon_tiles::fractional::FractionalHex;
use hexagon_tiles::layout::{Layout, LAYOUT_ORIENTATION_POINTY};
use hexagon_tiles::point::Point;
use hexagon_tiles::traits::HexRound;

use crate::coord::{TileHex, TileRange};

const HEX_GRID_LAYOUT: Layout = Layout {
    orientation: LAYOUT_ORIENTATION_POINTY,
    size: Point { x: 1.0, y: 1.0 },
    origin: Point { x: 0.0, y: 0.0 },
};

pub const FAR: Double = 0.0;

pub type Float = f32;

pub type Rad = cgmath::Rad<Float>;

pub fn rad(n: Float) -> Rad {
    cgmath::Rad(n)
}

pub type Deg = cgmath::Deg<Float>;

pub fn deg(n: Float) -> Deg {
    cgmath::Deg(n)
}

pub type Point1 = cgmath::Point1<Float>;
pub type Point2 = cgmath::Point2<Float>;
pub type Point3 = cgmath::Point3<Float>;

pub type Vector1 = cgmath::Vector1<Float>;
pub type Vector2 = cgmath::Vector2<Float>;
pub type Vector3 = cgmath::Vector3<Float>;
pub type Vector4 = cgmath::Vector4<Float>;

pub type Matrix2 = cgmath::Matrix2<Float>;
pub type Matrix3 = cgmath::Matrix3<Float>;
pub type Matrix4 = cgmath::Matrix4<Float>;

pub type Quaternion = cgmath::Quaternion<Float>;

pub type Double = f64;

pub type DPoint1 = cgmath::Point1<Double>;
pub type DPoint2 = cgmath::Point2<Double>;
pub type DPoint3 = cgmath::Point3<Double>;

pub type DVector1 = cgmath::Vector1<Double>;
pub type DVector2 = cgmath::Vector2<Double>;
pub type DVector3 = cgmath::Vector3<Double>;
pub type DVector4 = cgmath::Vector4<Double>;

pub type DMatrix2 = cgmath::Matrix2<Double>;
pub type DMatrix3 = cgmath::Matrix3<Double>;
pub type DMatrix4 = cgmath::Matrix4<Double>;

pub type DQuaternion = cgmath::Quaternion<Double>;

/// 0.01
#[inline]
pub fn z_near<N: BaseFloat>() -> N {
    let one = N::one();
    let two = one + one;
    let ten = two + two + two + two + two;

    one / ten.powi(2)
}

/// 10000
#[inline]
pub fn z_far<N: BaseFloat>() -> N {
    let one = N::one();
    let two = one + one;
    let ten = two + two + two + two + two;

    ten.powi(4)
}

#[rustfmt::skip]
pub fn perspective<N: BaseFloat>(fov_y: N, a: N, n: N, f: N) -> cgmath::Matrix4<N> {
    let zero = N::zero();
    let one = N::one();
    let two = one + one;

    let t = fov_y.div(two).tan();
    let d = f - n;
    let m = -(f * n);

    cgmath::Matrix4::<N>::new(
        one / (t * a), zero, zero, zero,
        zero, one / t, zero, zero,
        zero, zero, f / d, one,
        zero, zero, m / d, zero,
    )
}

pub fn projection<N: BaseFloat>(aspect: N, pi: N) -> cgmath::Matrix4<N> {
    let one = N::one();
    let two = one + one;

    perspective(pi / two, aspect, z_near(), z_far())
}

pub fn camera_angle<N: BaseFloat>(z: N) -> cgmath::Rad<N> {
    // TODO magic values
    let max = N::from(6.5).unwrap();

    if z < max {
        let normalized = (max - z) / N::from(4.0).unwrap();

        cgmath::Rad(normalized / N::from(-1.5).unwrap())
    } else {
        cgmath::Rad(N::zero())
    }
}

pub fn view<N: BaseFloat>(pos: cgmath::Point3<N>) -> cgmath::Matrix4<N> {
    cgmath::Matrix4::<N>::look_to_rh(
        pos,
        cgmath::Vector3::<N>::unit_z(),
        cgmath::Vector3::<N>::unit_y(),
    )
}

pub fn matrix<N: BaseFloat>(pos: cgmath::Point3<N>, aspect: N, pi: N) -> cgmath::Matrix4<N> {
    let projection = projection(aspect, pi);
    let view = view(pos);
    let angle = cgmath::Matrix4::<N>::from_angle_x(camera_angle(pos.z));

    projection * angle * view
}

#[inline]
pub fn pixel_to_hex<N: BaseFloat>(p: cgmath::Point2<N>) -> FractionalHex<Double> {
    hexagon_tiles::layout::pixel_to_hex(
        HEX_GRID_LAYOUT,
        hexagon_tiles::point::point(p.x.to_f64().unwrap(), p.y.to_f64().unwrap()),
    )
}

#[inline]
pub fn hex_to_pixel(hex: TileHex) -> DPoint2 {
    let p = hexagon_tiles::layout::hex_to_pixel(HEX_GRID_LAYOUT, hex);

    point2(p.x, p.y)
}

#[inline]
pub fn frac_hex_to_pixel(hex: FractionalHex<Double>) -> DPoint2 {
    let p = hexagon_tiles::layout::frac_hex_to_pixel(HEX_GRID_LAYOUT, hex);

    point2(p.x, p.y)
}

/// Converts screen space coordinates into normalized coordinates.
#[inline]
pub fn screen_to_normalized((width, height): (Double, Double), c: DPoint2) -> DPoint2 {
    let size = vec2(width, height) * 0.5;

    let c = vec2(c.x, c.y);
    let c = c.zip(size, Sub::sub);
    let c = c.zip(size, Div::div);

    point2(c.x, c.y)
}

/// Gets the hex position being pointed at.
#[inline]
pub fn main_pos_to_hex(
    (width, height): (Double, Double),
    main_pos: DPoint2,
    camera_pos: DPoint3,
) -> FractionalHex<Double> {
    let p = screen_to_world((width, height), main_pos, camera_pos);

    pixel_to_hex(point2(p.x, p.y))
}

/// Converts screen coordinates to world coordinates.
#[inline]
pub fn screen_to_world(
    (width, height): (Double, Double),
    pos: DPoint2,
    camera_pos: DPoint3,
) -> DPoint3 {
    let pos = screen_to_normalized((width, height), pos);

    normalized_to_world((width, height), pos, camera_pos)
}

/// Converts normalized screen coordinates to world coordinates.
#[inline]
pub fn normalized_to_world(
    (width, height): (Double, Double),
    pos: DPoint2,
    camera_pos: DPoint3,
) -> DPoint3 {
    let aspect = width / height;
    let aspect_squared = aspect * aspect;

    let eye = point3(0.0, 0.0, camera_pos.z);
    let matrix = matrix(eye, aspect, PI);

    let pos = vec3(pos.x, pos.y, FAR);
    let pos = matrix * pos.extend(1.0);
    let pos = pos.truncate() * pos.w;

    point3(pos.x * aspect_squared, pos.y, pos.z) + camera_pos.to_vec()
}

/// Gets the culling range from the camera's position
pub fn get_culling_range((width, height): (Double, Double), camera_pos: DPoint3) -> TileRange {
    let a = normalized_to_world((width, height), point2(-2.0, -2.0), camera_pos);
    let b = normalized_to_world((width, height), point2(2.0, 2.0), camera_pos);

    let a = pixel_to_hex(point2(a.x, a.y)).round().into();
    let b = pixel_to_hex(point2(b.x, b.y)).round().into();

    TileRange::new(a, b).extend(2)
}
#[inline]

pub fn direction_to_angle(d: DVector2) -> Rad {
    let angle = cgmath::Rad::atan2(d.y, d.x);

    rad(angle.0.rem_euclid(PI) as Float)
}
