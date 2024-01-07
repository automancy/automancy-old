use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use cgmath::{vec3, vec4, EuclideanSpace, Matrix, MetricSpace, SquareMatrix};
use egui::NumExt;
use egui_wgpu::wgpu::{
    vertex_attr_array, BufferAddress, VertexAttribute, VertexBufferLayout, VertexStepMode,
};
use gltf::animation::Interpolation;
use gltf::scene::Transform;
use hexagon_tiles::fractional::FractionalHex;
use hexagon_tiles::traits::HexRound;

use crate::coord::TileCoord;
use crate::math;
use crate::math::{
    direction_to_angle, DPoint2, Double, Float, Matrix3, Matrix4, Point3, Vector3, Vector4,
};

pub fn lerp_coords_to_pixel(a: TileCoord, b: TileCoord, t: Double) -> DPoint2 {
    let a = FractionalHex::new(a.q() as Double, a.r() as Double);
    let b = FractionalHex::new(b.q() as Double, b.r() as Double);
    let lerp = FractionalHex::lerp(a, b, t);

    math::frac_hex_to_pixel(lerp)
}

/// Produces a line shape.
pub fn make_line(a: DPoint2, b: DPoint2) -> Matrix4 {
    let mid = a.midpoint(b);
    let d = a.distance(b) as Float;
    let theta = direction_to_angle(b - a);

    Matrix4::from_translation(vec3(mid.x as Float, mid.y as Float, 0.1))
        * Matrix4::from_angle_z(theta)
        * Matrix4::from_nonuniform_scale(d.at_least(0.001), 0.1, 0.05)
}

// vertex

pub type VertexPos = [Float; 3];
pub type VertexColor = [Float; 4];
pub type RawMat4 = [[Float; 4]; 4];
pub type RawMat3 = [[Float; 3]; 3];

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialOrd, PartialEq, Zeroable, Pod)]
pub struct Vertex {
    pub pos: VertexPos,
    pub normal: VertexPos,
    pub color: VertexColor,
}

impl Vertex {
    pub fn desc() -> VertexBufferLayout<'static> {
        static ATTRIBUTES: &[VertexAttribute] = &vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x4,
        ];

        VertexBufferLayout {
            array_stride: size_of::<Vertex>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: ATTRIBUTES,
        }
    }
}

// instance

#[derive(Clone, Copy, Debug)]
pub struct InstanceData {
    color_offset: VertexColor,
    alpha: Float,
    light_pos: Vector4,
    model_matrix: Matrix4,
    projection: Option<Matrix4>,
}

impl Default for InstanceData {
    fn default() -> Self {
        Self {
            color_offset: [0.0, 0.0, 0.0, 0.0],
            alpha: 1.0,
            light_pos: vec4(0.0, 0.0, 0.0, 0.0),
            model_matrix: Matrix4::identity(),
            projection: None,
        }
    }
}

impl InstanceData {
    #[inline]
    pub fn add_model_matrix_left(mut self, model_matrix: Matrix4) -> Self {
        self.model_matrix = model_matrix * self.model_matrix;

        self
    }

    #[inline]
    pub fn add_model_matrix_right(mut self, model_matrix: Matrix4) -> Self {
        self.model_matrix = self.model_matrix * model_matrix;

        self
    }

    #[inline]
    pub fn add_translation(mut self, translation: Vector3) -> Self {
        self.model_matrix = self.model_matrix * Matrix4::from_translation(translation);

        self
    }

    #[inline]
    pub fn add_scale(mut self, scale: Float) -> Self {
        self.model_matrix = self.model_matrix * Matrix4::from_scale(scale);

        self
    }

    #[inline]
    pub fn with_model_matrix(mut self, model_matrix: Matrix4) -> Self {
        self.model_matrix = model_matrix;

        self
    }

    #[inline]
    pub fn add_alpha(mut self, alpha: Float) -> Self {
        self.alpha *= alpha;

        self
    }

    #[inline]
    pub fn with_alpha(mut self, alpha: Float) -> Self {
        self.alpha = alpha;

        self
    }

    #[inline]
    pub fn with_light_pos(mut self, light_pos: Point3, light_strength: Option<Float>) -> Self {
        self.light_pos = light_pos.to_vec().extend(light_strength.unwrap_or(1.0));

        self
    }

    #[inline]
    pub fn with_color_offset(mut self, color_offset: VertexColor) -> Self {
        self.color_offset = color_offset;

        self
    }

    #[inline]
    pub fn with_projection(mut self, projection: Matrix4) -> Self {
        self.projection = Some(projection);

        self
    }

    #[inline]
    pub fn add_projection_right(mut self, projection: Matrix4) -> Self {
        if let Some(s) = self.projection {
            self.projection = Some(s * projection);
        } else {
            self.projection = Some(projection);
        }

        self
    }

    #[inline]
    pub fn add_projection_left(mut self, projection: Matrix4) -> Self {
        if let Some(s) = self.projection {
            self.projection = Some(projection * s);
        } else {
            self.projection = Some(projection);
        }

        self
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Zeroable, Pod)]
pub struct RawInstanceData {
    color_offset: VertexColor,
    alpha: Float,
    light_pos: [Float; 4],
    model_matrix: RawMat4,
    normal_matrix: RawMat3,
}

impl From<InstanceData> for RawInstanceData {
    fn from(value: InstanceData) -> Self {
        let model_matrix = if let Some(projection) = value.projection {
            projection * value.model_matrix
        } else {
            value.model_matrix
        };

        let invert_transpose = value.model_matrix.invert().unwrap().transpose();

        Self {
            color_offset: value.color_offset,
            alpha: value.alpha,
            light_pos: [
                value.light_pos.x,
                value.light_pos.y,
                value.light_pos.z,
                value.light_pos.w,
            ],
            model_matrix: model_matrix.into(),
            normal_matrix: Matrix3::from_cols(
                invert_transpose.x.truncate(),
                invert_transpose.y.truncate(),
                invert_transpose.z.truncate(),
            )
            .into(),
        }
    }
}

impl RawInstanceData {
    pub fn desc() -> VertexBufferLayout<'static> {
        static ATTRIBUTES: &[VertexAttribute] = &vertex_attr_array![
            3 => Float32x4,
            4 => Float32,
            5 => Float32x4,
            6 => Float32x4,
            7 => Float32x4,
            8 => Float32x4,
            9 => Float32x4,
            10 => Float32x3,
            11 => Float32x3,
            12 => Float32x3,
        ];

        VertexBufferLayout {
            array_stride: size_of::<RawInstanceData>() as BufferAddress,
            step_mode: VertexStepMode::Instance,
            attributes: ATTRIBUTES,
        }
    }
}

// UBO

pub static DEFAULT_LIGHT_COLOR: VertexColor = [1.0; 4];

#[repr(C)]
#[derive(Clone, Copy, Debug, Zeroable, Pod)]
pub struct GameUBO {
    light_color: VertexColor,
    world_matrix: RawMat4,
}

static FIX_COORD: RawMat4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, -1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

impl Default for GameUBO {
    fn default() -> Self {
        Self {
            light_color: DEFAULT_LIGHT_COLOR,
            world_matrix: FIX_COORD,
        }
    }
}

impl GameUBO {
    pub fn new(world: Matrix4) -> Self {
        let world = Matrix4::from(FIX_COORD) * world;

        Self {
            world_matrix: world.into(),
            ..Default::default()
        }
    }
}

// model

#[derive(Debug, Clone)]
pub struct Model {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
    pub name: String,
    pub index: usize,
    pub matrix: Matrix4,
    pub transform: Transform,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Animation {
    pub target: usize,
    pub interpolation: Interpolation,
    pub inputs: Vec<Float>,
    pub outputs: Vec<Matrix4>,
}
