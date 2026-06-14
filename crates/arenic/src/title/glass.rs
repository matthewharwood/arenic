//! The title screen's **frosted-glass** post-process: a fullscreen blur +
//! FBM-warp + grain over the 3D layer, so the choreographed circles read as
//! soft, ethereal drop-shadows behind glass. The UI (title + buttons) composites
//! on top afterwards and stays sharp.
//!
//! This mirrors Bevy 0.18's `custom_post_processing` example verbatim in
//! structure: a [`ViewNode`] in the `Core3d` graph between `Tonemapping` and
//! `EndMainPassPostProcessing`, driven by a per-camera [`GlassSettings`] uniform
//! (extracted + uploaded by Bevy). Because the node's `ViewQuery` requires
//! [`GlassSettings`], it only runs on the title camera — the Intro camera (no
//! such component) is untouched.

use bevy::core_pipeline::FullscreenShader;
use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::extract_component::{
    ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
    UniformComponentPlugin,
};
use bevy::render::render_graph::{
    NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy::render::render_resource::{
    BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, CachedRenderPipelineId,
    ColorTargetState, ColorWrites, FragmentState, Operations, RenderPassColorAttachment,
    RenderPassDescriptor, RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor,
    ShaderStages, ShaderType, TextureFormat, TextureSampleType,
};
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::view::ViewTarget;
use bevy::render::{RenderApp, RenderStartup};

const SHADER_PATH: &str = "shaders/title_glass.wgsl";

/// Per-camera glass controls — lives in the main world, extracted + uploaded to
/// the GPU each frame. Present ONLY on the title camera, so the effect is
/// title-only. Packed into `vec4`s so the layout is 16-byte aligned (WebGL2).
#[derive(Component, Clone, Copy, ExtractComponent, ShaderType)]
pub(crate) struct GlassSettings {
    /// x = blur radius (texels), y = distort, z = noise scale, w = grain.
    pub params: Vec4,
    /// x = time (seconds); y/z/w spare.
    pub anim: Vec4,
    /// rgb = ethereal tint (a theme accent), a = tint strength.
    pub tint: Vec4,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct GlassLabel;

#[derive(Default)]
struct GlassNode;

impl ViewNode for GlassNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static GlassSettings,
        &'static DynamicUniformIndex<GlassSettings>,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, _settings, settings_index): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<GlassPipeline>();
        let pipeline_cache = world.resource::<bevy::render::render_resource::PipelineCache>();
        let Some(render_pipeline) = pipeline_cache.get_render_pipeline(pipeline.pipeline_id) else {
            return Ok(());
        };
        let uniforms = world.resource::<ComponentUniforms<GlassSettings>>();
        let Some(binding) = uniforms.uniforms().binding() else {
            return Ok(());
        };

        // Flip the view target: read `source`, write `destination`.
        let post_process = view_target.post_process_write();
        let bind_group = render_context.render_device().create_bind_group(
            "title_glass_bind_group",
            &pipeline_cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::sequential((
                post_process.source,
                &pipeline.sampler,
                binding.clone(),
            )),
        );

        let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("title_glass_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post_process.destination,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_render_pipeline(render_pipeline);
        pass.set_bind_group(0, &bind_group, &[settings_index.index()]);
        pass.draw(0..3, 0..1);
        Ok(())
    }
}

/// Global render-world data for the glass pipeline (built once at startup).
#[derive(Resource)]
struct GlassPipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

fn init_glass_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    fullscreen_shader: Res<FullscreenShader>,
    pipeline_cache: Res<bevy::render::render_resource::PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "title_glass_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<GlassSettings>(true),
            ),
        ),
    );
    let sampler = render_device.create_sampler(&SamplerDescriptor::default());
    let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("title_glass_pipeline".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen_shader.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: asset_server.load(SHADER_PATH),
            targets: vec![Some(ColorTargetState {
                // Assumes a NON-HDR title camera (LDR swapchain format). If the
                // title camera ever gains `Hdr`, this must follow the view's
                // ViewTarget format (Rgba16Float) instead.
                format: TextureFormat::bevy_default(),
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    });
    commands.insert_resource(GlassPipeline {
        layout,
        sampler,
        pipeline_id,
    });
}

/// Registers the glass post-process. Add to the app once; attach
/// [`GlassSettings`] to the camera you want it to run on.
pub(crate) struct GlassPlugin;

impl Plugin for GlassPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractComponentPlugin::<GlassSettings>::default(),
            UniformComponentPlugin::<GlassSettings>::default(),
        ));
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.add_systems(RenderStartup, init_glass_pipeline);
        render_app
            .add_render_graph_node::<ViewNodeRunner<GlassNode>>(Core3d, GlassLabel)
            .add_render_graph_edges(
                Core3d,
                (
                    Node3d::Tonemapping,
                    GlassLabel,
                    Node3d::EndMainPassPostProcessing,
                ),
            );
    }
}
