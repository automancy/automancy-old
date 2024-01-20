#![allow(unused_qualifications)]

use std::f64::consts::PI;
use std::ops::Div;

use glam::{dvec2, dvec3, vec2};
use hexx::{Hex, HexBounds, HexLayout, HexOrientation};

use crate::coord::TileCoord;

pub const HEX_GRID_LAYOUT: HexLayout = HexLayout {
    orientation: HexOrientation::Pointy,
    origin: Vec2::ZERO,
    hex_size: Vec2::ONE,
    invert_x: true,
    invert_y: true,
};

pub const FAR: Double = 0.0;

pub type Float = f32;

pub type Vec2 = glam::Vec2;
pub type Vec3 = glam::Vec3;
pub type Vec4 = glam::Vec4;

pub type Matrix2 = glam::Mat2;
pub type Matrix3 = glam::Mat3;
pub type Matrix4 = glam::Mat4;

pub type Quaternion = glam::Quat;

pub type Double = f64;

pub type DVec2 = glam::DVec2;
pub type DVec3 = glam::DVec3;
pub type DVec4 = glam::DVec4;

pub type DMatrix2 = glam::DMat2;
pub type DMatrix3 = glam::DMat3;
pub type DMatrix4 = glam::DMat4;

pub type DQuaternion = glam::DQuat;

#[inline]
pub fn z_near() -> Double {
    0.01
}

#[inline]
pub fn z_far() -> Double {
    10000.0
}

#[rustfmt::skip]
pub fn perspective(fov_y: Double, a: Double, n: Double, f: Double) -> DMatrix4 {
    let t = fov_y.div(2.0).tan();
    let d = f - n;
    let m = -(f * n);

    DMatrix4::from_cols_array(&[
        1.0 / (t * a), 0.0, 0.0, 0.0,
        0.0, 1.0 / t, 0.0, 0.0,
        0.0, 0.0, f / d, 1.0,
        0.0, 0.0, m / d, 0.0,
    ])
}

pub fn projection(aspect: Double, pi: Double) -> DMatrix4 {
    perspective(pi / 2.0, aspect, z_near(), z_far())
}

pub fn camera_angle(z: Double) -> Double {
    // TODO magic values
    let max = 6.5;

    if z < max {
        let normalized = (max - z) / 4.0;

        normalized / -1.5
    } else {
        0.0
    }
}

pub fn view(pos: DVec3) -> DMatrix4 {
    DMatrix4::look_to_rh(pos, dvec3(0.0, 0.0, 1.0), dvec3(0.0, 1.0, 0.0))
}

pub fn matrix(pos: DVec3, aspect: Double, pi: Double) -> DMatrix4 {
    let projection = projection(aspect, pi);
    let view = view(pos);
    let angle = DMatrix4::from_rotation_x(camera_angle(pos.z));

    projection * angle * view
}

pub fn lerp_coords_to_pixel(a: TileCoord, b: TileCoord, t: Float) -> Vec2 {
    let a = Vec2::new(a.x as Float, a.y as Float);
    let b = Vec2::new(b.x as Float, b.y as Float);
    let lerp = Vec2::lerp(a, b, t);

    let p = HEX_GRID_LAYOUT.fractional_hex_to_world_pos(lerp);

    vec2(p.x, p.y)
}

/// Converts screen space coordinates into normalized coordinates.
#[inline]
pub fn screen_to_normalized((width, height): (Double, Double), c: DVec2) -> DVec2 {
    let size = dvec2(width, height) * 0.5;

    let c = dvec2(c.x, c.y);
    let c = c - size;
    let c = c / size;

    dvec2(c.x, c.y)
}

/// Gets the hex position being pointed at.
#[inline]
pub fn main_pos_to_hex(
    (width, height): (Double, Double),
    main_pos: DVec2,
    camera_pos: DVec3,
) -> Hex {
    let p = screen_to_world((width, height), main_pos, camera_pos);

    HEX_GRID_LAYOUT.world_pos_to_hex(vec2(p.x as Float, p.y as Float))
}

/// Converts screen coordinates to world coordinates.
#[inline]
pub fn screen_to_world((width, height): (Double, Double), pos: DVec2, camera_pos: DVec3) -> DVec3 {
    let pos = screen_to_normalized((width, height), pos);

    normalized_to_world((width, height), pos, camera_pos)
}

/// Converts normalized screen coordinates to world coordinates.
#[inline]
pub fn normalized_to_world(
    (width, height): (Double, Double),
    pos: DVec2,
    camera_pos: DVec3,
) -> DVec3 {
    let aspect = width / height;
    let aspect_squared = aspect * aspect;

    let eye = dvec3(0.0, 0.0, camera_pos.z);
    let matrix = matrix(eye, aspect, PI);

    let pos = dvec3(pos.x, pos.y, FAR);
    let pos = matrix * pos.extend(1.0);
    let pos = pos.truncate() * pos.w;

    dvec3(pos.x * aspect_squared, pos.y, pos.z) + camera_pos
}

/// Gets the culling range from the camera's position
pub fn get_culling_range((width, height): (Double, Double), camera_pos: DVec3) -> HexBounds {
    HexBounds::new(
        HEX_GRID_LAYOUT.world_pos_to_hex(vec2(camera_pos.x as Float, camera_pos.y as Float)),
        ((width / height) * camera_pos.z.round()) as u32 + 8,
    )
}
#[inline]
pub fn direction_to_angle(d: Vec2) -> Float {
    let angle = d.y.atan2(d.x);

    angle.rem_euclid(std::f32::consts::PI)
}
