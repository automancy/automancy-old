use std::sync::Arc;

use automancy_defs::cg::{DPoint2, Float};
use automancy_defs::cgmath;
use automancy_defs::cgmath::MetricSpace;
use automancy_defs::egui::{vec2, PaintCallback, Rgba, ScrollArea, Ui};
use automancy_defs::egui_winit_vulkano::CallbackFn;
use automancy_defs::id::Id;
use automancy_defs::rendering::{GameVertex, InstanceData, LightInfo};
use automancy_resources::data::item::Item;
use automancy_resources::ResourceManager;
use fuse_rust::Fuse;
use genmesh::{EmitTriangles, Quad};
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage};

use automancy::render::gpu;
use automancy::render::renderer::Renderer;

pub mod tile_config;

/// Draws an Item's icon.
pub fn draw_item(
    ui: &mut Ui,
    resource_man: Arc<ResourceManager>,
    renderer: &Renderer,
    item: Item,
    size: Float,
) {
    let model = if resource_man.meshes.contains_key(&item.model) {
        item.model
    } else {
        resource_man.registry.model_ids.items_missing
    };

    let (_, rect) = ui.allocate_space(vec2(size, size));

    let pipeline = renderer.gpu.gui_pipeline.clone();
    let vertex_buffer = renderer.gpu.alloc.vertex_buffer.clone();
    let index_buffer = renderer.gpu.alloc.index_buffer.clone();

    let callback = PaintCallback {
        rect,
        callback: Arc::new(CallbackFn::new(move |_info, context| {
            let instance = (InstanceData::default().into(), model);

            let light_info = Buffer::from_data(
                &context.resources.memory_allocator,
                BufferCreateInfo {
                    usage: BufferUsage::VERTEX_BUFFER,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    usage: MemoryUsage::Upload,
                    ..Default::default()
                },
                LightInfo {
                    light_pos: [0.0, 0.0, 2.0],
                    light_color: [1.0; 4],
                },
            )
            .unwrap();

            if let Some((indirect_commands, instance_buffer)) = gpu::indirect_instance(
                &context.resources.memory_allocator,
                &resource_man,
                &[instance],
            ) {
                context
                    .builder
                    .bind_pipeline_graphics(pipeline.clone())
                    .bind_vertex_buffers(0, (vertex_buffer.clone(), instance_buffer, light_info))
                    .bind_index_buffer(index_buffer.clone())
                    .draw_indexed_indirect(indirect_commands)
                    .unwrap();
            }
        })),
    };

    ui.painter().add(callback);
}

/// Produces a line shape.
pub fn make_line(a: DPoint2, b: DPoint2, color: Rgba) -> Vec<GameVertex> {
    let v = b - a;
    let l = a.distance(b) * 128.0;
    let w = cgmath::vec2(-v.y / l, v.x / l);

    let a0 = (a + w).cast::<Float>().unwrap();
    let a1 = (b + w).cast::<Float>().unwrap();
    let b0 = (b - w).cast::<Float>().unwrap();
    let b1 = (a - w).cast::<Float>().unwrap();

    let mut line = vec![];

    Quad::new(
        GameVertex {
            pos: [a0.x, a0.y, 0.0],
            color: color.to_array(),
            normal: [0.0, 0.0, 0.0],
        },
        GameVertex {
            pos: [a1.x, a1.y, 0.0],
            color: color.to_array(),
            normal: [0.0, 0.0, 0.0],
        },
        GameVertex {
            pos: [b0.x, b0.y, 0.0],
            color: color.to_array(),
            normal: [0.0, 0.0, 0.0],
        },
        GameVertex {
            pos: [b1.x, b1.y, 0.0],
            color: color.to_array(),
            normal: [0.0, 0.0, 0.0],
        },
    )
    .emit_triangles(|v| line.append(&mut vec![v.x, v.y, v.z]));

    line
}

/// Draws a search bar.
pub fn searchable_id<'a>(
    ui: &mut Ui,
    resource_man: &'a ResourceManager,
    fuse: &Fuse,
    ids: &[Id],
    new_id: &mut Option<Id>,
    filter: &mut String,
    name: &'static impl Fn(&'a ResourceManager, &Id) -> &'a str,
) {
    ui.text_edit_singleline(filter);

    ScrollArea::vertical().max_height(80.0).show(ui, |ui| {
        ui.set_width(ui.available_width());

        let ids = if !filter.is_empty() {
            let mut filtered = ids
                .iter()
                .flat_map(|id| {
                    let result = fuse.search_text_in_string(filter, name(resource_man, id));
                    let score = result.map(|v| v.score);

                    if score.unwrap_or(0.0) > 0.4 {
                        None
                    } else {
                        Some(*id).zip(score)
                    }
                })
                .collect::<Vec<_>>();

            filtered.sort_unstable_by(|a, b| a.1.total_cmp(&b.1));

            filtered.into_iter().map(|v| v.0).collect::<Vec<_>>()
        } else {
            ids.to_vec()
        };

        ids.iter().for_each(|script| {
            ui.radio_value(new_id, Some(*script), name(resource_man, script));
        })
    });
}