extern crate nuklear;

#[macro_use]
extern crate gfx;

use nuklear::{Buffer, Context, ConvertConfig, DrawVertexLayoutAttribute, DrawVertexLayoutElements, DrawVertexLayoutFormat, Handle, Vec2};

use gfx::format::{R8_G8_B8_A8, U8Norm, Unorm};
use gfx::handle::{Buffer as GfxBuffer, RenderTargetView, Sampler, ShaderResourceView};
use gfx::texture::{AaMode, Kind, Mipmap};
use gfx::traits::FactoryExt;
use gfx::{Encoder, Factory, Resources};

pub type ColorFormat = gfx::format::Rgba8;

pub enum GfxBackend {
    OpenGlsl150,
    DX11Hlsl,
}

gfx_defines!{
    vertex Vertex {
        pos: [f32; 2] = "Position",
        tex: [f32; 2] = "TexCoord",
        col: [U8Norm; 4] = "Color",
    }

    constant Locals {
        proj: [[f32; 4]; 4] = "ProjMtx",
    }

    pipeline pipe {
        vbuf: gfx::VertexBuffer<Vertex> = (),
        tex: gfx::TextureSampler<[f32; 4]> = "Texture",
        output: gfx::BlendTarget<super::ColorFormat> = ("Target0", gfx::state::ColorMask::all(), gfx::preset::blend::ALPHA),
        locals: gfx::ConstantBuffer<Locals> = "Locals",
        scissors: gfx::Scissor = (),
    }
}

impl Default for Vertex {
    fn default() -> Self {
        unsafe { ::std::mem::zeroed() }
    }
}

pub struct Drawer<R: Resources> {
    cmd: Buffer,
    pso: gfx::PipelineState<R, pipe::Meta>,
    smp: Sampler<R>,
    tex: Vec<ShaderResourceView<R, [f32; 4]>>,
    vbf: GfxBuffer<R, Vertex>,
    ebf: GfxBuffer<R, u16>,
    lbf: GfxBuffer<R, Locals>,
    vsz: usize,
    esz: usize,
    vle: DrawVertexLayoutElements,

    pub col: Option<RenderTargetView<R, (R8_G8_B8_A8, Unorm)>>,
}

impl<R: gfx::Resources> Drawer<R> {
    pub fn new<F>(factory: &mut F, col: RenderTargetView<R, (R8_G8_B8_A8, Unorm)>, texture_count: usize, vbo_size: usize, ebo_size: usize, command_buffer: Buffer, backend: GfxBackend) -> Drawer<R>
    where
        F: Factory<R>,
    {
        use gfx::pso::buffer::Structure;

        let vs: &[u8] = match backend {
            GfxBackend::OpenGlsl150 => include_bytes!("../shaders/glsl_150/vs.glsl"),
            GfxBackend::DX11Hlsl => include_bytes!("../shaders/hlsl/vs.fx"),
        };

        let fs: &[u8] = match backend {
            GfxBackend::OpenGlsl150 => include_bytes!("../shaders/glsl_150/fs.glsl"),
            GfxBackend::DX11Hlsl => include_bytes!("../shaders/hlsl/ps.fx"),
            //_ => &[0u8; 0],
        };

        Drawer {
            cmd: command_buffer,
            col: Some(col),
            smp: factory.create_sampler_linear(),
            pso: factory.create_pipeline_simple(vs, fs, pipe::new()).unwrap(),
            tex: Vec::with_capacity(texture_count + 1),
            vbf: factory.create_upload_buffer::<Vertex>(vbo_size).unwrap(),
            ebf: factory.create_upload_buffer::<u16>(ebo_size).unwrap(),
            vsz: vbo_size,
            esz: ebo_size,
            lbf: factory.create_constant_buffer::<Locals>(1),
            vle: DrawVertexLayoutElements::new(&[
                (DrawVertexLayoutAttribute::NK_VERTEX_POSITION, DrawVertexLayoutFormat::NK_FORMAT_FLOAT, Vertex::query("Position").unwrap().offset),
                (DrawVertexLayoutAttribute::NK_VERTEX_TEXCOORD, DrawVertexLayoutFormat::NK_FORMAT_FLOAT, Vertex::query("TexCoord").unwrap().offset),
                (DrawVertexLayoutAttribute::NK_VERTEX_COLOR, DrawVertexLayoutFormat::NK_FORMAT_R8G8B8A8, Vertex::query("Color").unwrap().offset),
                (DrawVertexLayoutAttribute::NK_VERTEX_ATTRIBUTE_COUNT, DrawVertexLayoutFormat::NK_FORMAT_COUNT, 0u32),
            ]),
        }
    }

    pub fn add_texture<F>(&mut self, factory: &mut F, image: &[u8], width: u32, height: u32) -> Handle
    where
        F: Factory<R>,
    {
        let (_, view) = factory.create_texture_immutable_u8::<ColorFormat>(Kind::D2(width as u16, height as u16, AaMode::Single), Mipmap::Provided, &[image]).unwrap();

        self.tex.push(view);

        Handle::from_id(self.tex.len() as i32)
    }

    pub fn draw<F, B: gfx::CommandBuffer<R>>(&mut self, ctx: &mut Context, cfg: &mut ConvertConfig, encoder: &mut Encoder<R, B>, factory: &mut F, width: u32, height: u32, scale: Vec2)
    where
        F: Factory<R>,
    {
        use gfx::IntoIndexBuffer;

        if self.col.clone().is_none() {
            return;
        }

        let ortho = [
            [2.0f32 / width as f32, 0.0f32, 0.0f32, 0.0f32],
            [0.0f32, -2.0f32 / height as f32, 0.0f32, 0.0f32],
            [0.0f32, 0.0f32, -1.0f32, 0.0f32],
            [-1.0f32, 1.0f32, 0.0f32, 1.0f32],
        ];

        cfg.set_vertex_layout(&self.vle);
        cfg.set_vertex_size(::std::mem::size_of::<Vertex>());

        {
            let mut rwv = factory.write_mapping(&mut self.vbf).unwrap();
            let mut rvbuf = unsafe { ::std::slice::from_raw_parts_mut(&mut *rwv as *mut [Vertex] as *mut u8, ::std::mem::size_of::<Vertex>() * self.vsz) };
            let mut vbuf = Buffer::with_fixed(&mut rvbuf);

            let mut rwe = factory.write_mapping(&mut self.ebf).unwrap();
            let mut rebuf = unsafe { ::std::slice::from_raw_parts_mut(&mut *rwe as *mut [u16] as *mut u8, ::std::mem::size_of::<u16>() * self.esz) };
            let mut ebuf = Buffer::with_fixed(&mut rebuf);

            ctx.convert(&mut self.cmd, &mut vbuf, &mut ebuf, cfg);
        }

        let mut slice = ::gfx::Slice {
            start: 0,
            end: 0,
            base_vertex: 0,
            instances: None,
            buffer: self.ebf.clone().into_index_buffer(factory),
        };

        encoder.update_constant_buffer(&mut self.lbf, &Locals { proj: ortho });

        for cmd in ctx.draw_command_iterator(&self.cmd) {
            if cmd.elem_count() < 1 {
                continue;
            }

            slice.end = slice.start + cmd.elem_count();

            let id = cmd.texture().id().unwrap();

            let x = cmd.clip_rect().x * scale.x;
            let y = cmd.clip_rect().y * scale.y;
            let w = cmd.clip_rect().w * scale.x;
            let h = cmd.clip_rect().h * scale.y;

            let sc_rect = gfx::Rect {
                x: (if x < 0f32 { 0f32 } else { x }) as u16,
                y: (if y < 0f32 { 0f32 } else { y }) as u16,
                w: (if x < 0f32 { w + x } else { w }) as u16,
                h: (if y < 0f32 { h + y } else { h }) as u16,
            };

            let res = self.find_res(id).unwrap();

            let data = pipe::Data {
                vbuf: self.vbf.clone(),
                tex: (res, self.smp.clone()),
                output: self.col.clone().unwrap(),
                scissors: sc_rect,
                locals: self.lbf.clone(),
            };

            encoder.draw(&slice, &self.pso, &data);

            slice.start = slice.end;
        }
    }

    fn find_res(&self, id: i32) -> Option<ShaderResourceView<R, [f32; 4]>> {
        let mut ret = None;

        if id > 0 && id as usize <= self.tex.len() {
            ret = Some(self.tex[(id - 1) as usize].clone());
        }

        ret
    }
}
