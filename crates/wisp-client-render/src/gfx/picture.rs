//! Wisp's title backdrop: a flat picture from the player's own backgrounds folder (the one the
//! launcher's backdrop shows), drawn over the whole screen in place of the panorama cube and
//! blurred with it. Pictures are never shipped with Wisp: they are the player's.
use crate::hud::decode_image_bytes;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use wgpu::util::DeviceExt;

/// A decoded picture, top row first.
pub struct Picture {
    pub width: u32,
    pub height: u32,
    rgba: Vec<u8>,
}

impl std::fmt::Debug for Picture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Picture({}x{})", self.width, self.height)
    }
}

/// The largest side a picture keeps: bigger ones are halved until they fit, so a photo
/// straight from a camera doesn't fill video memory behind a blurred menu.
const MAX_SIDE: u32 = 2560;

impl Picture {
    /// A PNG, JPEG or WebP file.
    pub fn load(path: &Path) -> Option<Self> {
        let data = std::fs::read(path).ok()?;
        let (mut width, mut height, mut rgba) = decode_image_bytes(&data)
            .map_err(|e| log::warn!("Wisp: background {}: {e}", path.display()))
            .ok()?;
        while width > MAX_SIDE || height > MAX_SIDE {
            (width, height, rgba) = halve(width, height, &rgba);
        }
        (width > 0 && height > 0).then_some(Self {
            width,
            height,
            rgba,
        })
    }
}

/// The picture at half its size, each texel the mean of four.
fn halve(width: u32, height: u32, rgba: &[u8]) -> (u32, u32, Vec<u8>) {
    let (w, h) = ((width / 2).max(1), (height / 2).max(1));
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            for c in 0..4 {
                let at = |dx: u32, dy: u32| {
                    let (sx, sy) = ((x * 2 + dx).min(width - 1), (y * 2 + dy).min(height - 1));
                    u32::from(rgba[((sy * width + sx) * 4 + c) as usize])
                };
                out.push(((at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1)) / 4) as u8);
            }
        }
    }
    (w, h, out)
}

/// The pictures in a backgrounds folder, by name.
pub fn pictures(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|x| x.to_str()).is_some_and(|x| {
                matches!(
                    x.to_ascii_lowercase().as_str(),
                    "png" | "jpg" | "jpeg" | "webp"
                )
            })
        })
        .collect();
    found.sort();
    found
}

/// The UV scale and offset that crop a picture to cover the screen, zoomed in by `zoom`.
fn cover(screen: (u32, u32), picture: (u32, u32), zoom: f32) -> [f32; 4] {
    let screen_aspect = screen.0 as f32 / screen.1.max(1) as f32;
    let picture_aspect = picture.0 as f32 / picture.1.max(1) as f32;
    let (sx, sy) = if screen_aspect > picture_aspect {
        (1.0, picture_aspect / screen_aspect)
    } else {
        (screen_aspect / picture_aspect, 1.0)
    };
    let (sx, sy) = (sx / zoom.max(1.0), sy / zoom.max(1.0));
    [sx, sy, (1.0 - sx) / 2.0, (1.0 - sy) / 2.0]
}

pub(super) struct TitlePicture {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    /// The picture on the GPU and its bindings, while the title shows one.
    shown: Option<(Arc<Picture>, wgpu::BindGroup)>,
}

impl TitlePicture {
    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Title picture filtering"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Title picture fit"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Title picture bindings"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Title picture shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("picture.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Title picture layout"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Title picture"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_picture"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_picture"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            layout,
            sampler,
            uniform,
            shown: None,
        }
    }

    /// Ready this frame's picture (none: the panorama shows); returns whether one is drawn.
    pub(super) fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: (u32, u32),
        picture: Option<&Arc<Picture>>,
        zoom: f32,
    ) -> bool {
        let Some(picture) = picture else {
            self.shown = None;
            return false;
        };
        let limit = device.limits().max_texture_dimension_2d;
        if picture.width > limit || picture.height > limit {
            self.shown = None;
            return false;
        }
        if !self
            .shown
            .as_ref()
            .is_some_and(|(p, _)| Arc::ptr_eq(p, picture))
        {
            self.shown = Some((picture.clone(), self.bind(device, queue, picture)));
        }
        let fit = cover(screen, (picture.width, picture.height), zoom);
        queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(&fit));
        true
    }

    /// Give the picture's video memory back.
    pub(super) fn release(&mut self) {
        self.shown = None;
    }

    fn bind(&self, device: &wgpu::Device, queue: &wgpu::Queue, p: &Picture) -> wgpu::BindGroup {
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("Title picture"),
                size: wgpu::Extent3d {
                    width: p.width,
                    height: p.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &p.rgba,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Title picture resources"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    /// Draw the picture `prepare` readied; returns whether there was one.
    pub(super) fn draw(&self, pass: &mut wgpu::RenderPass<'_>) -> bool {
        let Some((_, group)) = &self.shown else {
            return false;
        };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, group, &[]);
        pass.draw(0..3, 0..1);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_screen_crops_a_square_picture_top_and_bottom() {
        let [sx, sy, ox, oy] = cover((1600, 900), (1000, 1000), 1.0);
        assert_eq!((sx, ox), (1.0, 0.0));
        assert!((sy - 0.5625).abs() < 1e-6);
        assert!((oy - 0.21875).abs() < 1e-6);
    }

    #[test]
    fn a_tall_screen_crops_a_wide_picture_at_the_sides() {
        let [sx, sy, _, oy] = cover((900, 1600), (1600, 900), 1.0);
        assert_eq!((sy, oy), (1.0, 0.0));
        assert!(sx < 0.33);
    }

    #[test]
    fn zoom_shows_less_of_the_picture_around_its_middle() {
        let [sx, sy, ox, oy] = cover((1000, 1000), (1000, 1000), 2.0);
        assert_eq!([sx, sy, ox, oy], [0.5, 0.5, 0.25, 0.25]);
    }

    #[test]
    fn halving_averages_four_texels() {
        let rgba = [
            0, 0, 0, 255, 200, 200, 200, 255, 100, 100, 100, 255, 100, 100, 100, 255,
        ];
        assert_eq!(halve(2, 2, &rgba), (1, 1, vec![100, 100, 100, 255]));
    }
}
