extern crate nuklear_rust;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate gfx;

use nuklear_rust::{NkHandle, NkContext, NkConvertConfig, NkVec2, NkBuffer,
					NkDrawVertexLayoutElements, NkDrawVertexLayoutAttribute, NkDrawVertexLayoutFormat};

use gfx::{Factory, Resources, Encoder};
use gfx::tex::{Kind, AaMode};
use gfx::format::{R8_G8_B8_A8, Unorm, U8Norm};
use gfx::handle::{ShaderResourceView, RenderTargetView, Sampler, Buffer};
use gfx::traits::FactoryExt;

type DepthFormat = gfx::format::DepthStencil;

pub type ColorFormat = gfx::format::Rgba8;

gfx_defines!{
    vertex Vertex {
	    pos: [f32; 2] = "Position",
	    tex: [f32; 2] = "TexCoord",
	    col: [U8Norm; 4] = "Color",
	}

    pipeline pipe {
	    vbuf: gfx::VertexBuffer<Vertex> = (),
	    proj: gfx::Global<[[f32; 4]; 4]> = "ProjMtx",
	    tex: gfx::TextureSampler<[f32; 4]> = "Texture",
	    output: gfx::BlendTarget<super::ColorFormat> = ("Out_Color", gfx::state::MASK_ALL, gfx::preset::blend::ALPHA),
	    scissors: gfx::Scissor = (),
	}
}

impl Default for Vertex {
	fn default() -> Self { unsafe { ::std::mem::zeroed() } }
}

pub struct Drawer<R: Resources> {
	cmd: NkBuffer,
	pso: gfx::PipelineState<R, pipe::Meta>,    			
	col: RenderTargetView<R, (R8_G8_B8_A8, Unorm)>,
	smp: Sampler<R>,
	tex: Vec<ShaderResourceView<R, [f32; 4]>>,
	vbf: Buffer<R, Vertex>,
	ebf: Buffer<R, u16>,
	vsz: usize,
	esz: usize,
	vle: NkDrawVertexLayoutElements,
}

impl <R: gfx::Resources> Drawer<R> {
	pub fn new<F>(
				factory: &mut F, 
				col: &RenderTargetView<R, (R8_G8_B8_A8, Unorm)>, 
				texture_count: usize, 
				vbo_size: usize,
				ebo_size: usize, 
				command_buffer: NkBuffer
	) -> Drawer<R> where F: Factory<R> {
		use gfx::pso::buffer::Structure;
		
		Drawer {
			cmd: command_buffer, 
			col: col.clone(),
			smp: factory.create_sampler_linear(),
			pso: factory.create_pipeline_simple(
					include_bytes!("../shaders/glsl_150/vs.glsl"),
					include_bytes!("../shaders/glsl_150/fs.glsl"),
					pipe::new()
				).unwrap(),
			tex: Vec::with_capacity(texture_count + 1),
			vbf: factory.create_buffer_dynamic::<Vertex>(vbo_size, ::gfx::BufferRole::Vertex, ::gfx::Bind::empty()).unwrap(),
			ebf: factory.create_buffer_dynamic::<u16>(ebo_size, ::gfx::BufferRole::Index, ::gfx::Bind::empty()).unwrap(),
			vsz: vbo_size,
			esz: ebo_size,
			vle: NkDrawVertexLayoutElements::new(&[
		            (NkDrawVertexLayoutAttribute::NK_VERTEX_POSITION, NkDrawVertexLayoutFormat::NK_FORMAT_FLOAT, Vertex::query("Position").unwrap().offset),
		            (NkDrawVertexLayoutAttribute::NK_VERTEX_TEXCOORD, NkDrawVertexLayoutFormat::NK_FORMAT_FLOAT, Vertex::query("TexCoord").unwrap().offset),
		            (NkDrawVertexLayoutAttribute::NK_VERTEX_COLOR, NkDrawVertexLayoutFormat::NK_FORMAT_R8G8B8A8, Vertex::query("Color").unwrap().offset),
		            (NkDrawVertexLayoutAttribute::NK_VERTEX_ATTRIBUTE_COUNT, NkDrawVertexLayoutFormat::NK_FORMAT_COUNT, 0u32)
		        ]),
		}
	}
	
	pub fn add_texture<F>(&mut self, factory: &mut F, image: &[u8], width: u32, height: u32) -> NkHandle where F: Factory<R> {
		let (_, view) = factory.create_texture_const_u8::<ColorFormat>(Kind::D2(width as u16, height as u16, AaMode::Single), &[image]).unwrap();
	    
	    self.tex.push(view);
	    
	    NkHandle::from_id(self.tex.len() as i32)
	}
	
	pub fn draw<F ,B: gfx::CommandBuffer<R>>(&mut self, ctx: &mut NkContext, cfg: &mut NkConvertConfig, encoder: &mut Encoder<R,B>, factory: &mut F, tmp: &mut [u16], width: u32, height: u32, scale: NkVec2) where F: Factory<R> {
		//use gfx::pso::buffer::Structure;
		use gfx::IntoIndexBuffer;  
		
		let ortho = [[2.0f32 / width as f32, 0.0f32, 				 0.0f32, 0.0f32],
					 [0.0f32,				-2.0f32 / height as f32, 0.0f32, 0.0f32],
					 [0.0f32, 				 0.0f32,				-1.0f32, 0.0f32],
					 [-1.0f32,				 1.0f32, 				 0.0f32, 1.0f32]];      
					 
		cfg.set_vertex_layout(&self.vle);
		cfg.set_vertex_size(::std::mem::size_of::<Vertex>());			 
	
		{
			let mut rwv = factory.map_buffer_rw(&mut self.vbf);
			let mut rvbuf = unsafe { ::std::slice::from_raw_parts_mut(&mut *rwv as *mut [Vertex] as *mut u8, ::std::mem::size_of::<Vertex>() * self.vsz) }; 
			let mut vbuf = NkBuffer::with_fixed(&mut rvbuf);
			
			let mut rebuf = unsafe { ::std::slice::from_raw_parts_mut(tmp as *mut [u16] as *mut u8, ::std::mem::size_of::<u16>() * self.esz) }; 
			let mut ebuf = NkBuffer::with_fixed(&mut rebuf);
			
			ctx.convert(&mut self.cmd, &mut vbuf, &mut ebuf, cfg);	
		}
		
		{
			let mut rwe = factory.map_buffer_rw(&mut self.ebf);//TODO remove with gfx update
			(&mut *rwe).clone_from_slice(tmp);
		}
		
		let mut slice = ::gfx::Slice {
		    start: 0,
		    end: 0,
		    base_vertex: 0,
		    instances: None,
		    buffer: self.ebf.clone().into_index_buffer(factory),
		};
		
		//let mut cmd1 = ctx.draw_begin(&mut dev.cmds);
		
		//while let Some(cmd) = cmd1.clone() {
		
		for cmd in ctx.draw_command_iterator(&self.cmd) {
			
			if cmd.elem_count() < 1 { 
				continue; 
			}
			
			//println!("{:?}", cmd);
			
			 
			slice.end = slice.start + cmd.elem_count();
			
			let id = cmd.texture().id().unwrap();
			
			let x = cmd.clip_rect().x * scale.x;
			let y = (height as f32 - (cmd.clip_rect().y + cmd.clip_rect().h)) * scale.y;
			let w = cmd.clip_rect().w * scale.x;
			let h = cmd.clip_rect().h * scale.y;
			
			let sc_rect = gfx::Rect{
		    	x: (if x < 0f32 { 0f32 } else { x }) as u16,
		    	y: (if y < 0f32 { 0f32 } else { y }) as u16,
		    	w: (if x < 0f32 { w+x } else { w }) as u16,
		    	h: (if y < 0f32 { h+y } else { h }) as u16,
		    };
			
			let res = self.find_res(id).unwrap();
			
			let data = pipe::Data {
				vbuf: self.vbf.clone(),
			    proj: ortho,
			    tex: (res, self.smp.clone()), 
			    output: self.col.clone(),
			    scissors: sc_rect,
			};
			
			encoder.draw(&slice, &self.pso, &data);
			
			slice.start = slice.end;
			
			//cmd1 = ctx.draw_next(&cmd, &dev.cmds);
		}	
	}
	
	fn find_res(&self, id: i32) -> Option<ShaderResourceView<R, [f32; 4]>> {
		let mut ret = None;
		
		if id > 0 && id as usize <= self.tex.len() {
			ret = Some(self.tex[(id-1) as usize].clone());
		}
		
		ret
	}
}