/**
 * This recipe uses the vertex shader to create a ripple effect on a planar
 * mesh.
 **/

#[macro_use]
extern crate glium;
extern crate glutin;
extern crate glCookbook;
extern crate nalgebra;
extern crate num;

use glutin::{Event};
use glium::{DisplayBuild, Surface, Display};
use glium::index::{NoIndices, PrimitiveType};
use glCookbook::{
    BaseVertex, Grid, FreeCamera, RenderableVertex,
    RenderableIndices, Renderable, NormalVertex,
    Controller, IsoSphere
};
use nalgebra::{Vec3, Mat4, Iso3, Transformation, RotationMatrix};
use num::Float;

// Program entry point
fn main() {
    let display = glutin::WindowBuilder::new()
        .with_dimensions(1366, 768)
        .with_multisampling(4)
        .with_depth_buffer(24)
        .with_vsync()
        .with_title("tetra".to_string())
        .build_glium()
        .unwrap();

    let ball = IsoSphere::new(3);
    let grid = Grid::new(20.0, 20.0, 20, 20);
    let ball_model =
        nalgebra::Iso3::new(Vec3::new(0.0, 2.0, 0.0), nalgebra::zero());


    implement_vertex!(Vertex, position, normal);
    let mut lighting_renderer = LightingRenderer::new(&display);
    let normal_renderer       = NormalRenderer::new(&display);
    let mut camera            = FreeCamera::new(1.0, 75.0, 1.0, 500.0);
    let mut time = 0.0f32;

    camera.pos.y = 2.0;
    lighting_renderer.light_position = Vec3::new(10.0, 2.0, 0.0);
    lighting_renderer.diffuse_color = Vec3::new(0.2, 0.2, 0.8);
    lighting_renderer.specular_color = Vec3::new(0.8, 0.8, 0.8);
    lighting_renderer.shininess = 256.0;


    let mut controller = Controller::new();
    controller.rot_speed = 1.0/40.0;
    controller.move_speed = 0.2;

    'mainLoop : loop {
        time += 0.02;
        let (x, z) = time.sin_cos();
        lighting_renderer.light_position = Vec3::new(x*10.0, 2.0, z*10.0);

        let mut target = display.draw();
        target.clear_color_and_depth((0.02, 0.02, 0.05, 1.0), 1.0);


        lighting_renderer.draw(
            &mut target, &grid, &camera.projection.to_mat(),
            &camera.get_view_transform(), &Iso3::new(nalgebra::zero(), nalgebra::zero())
        );

        lighting_renderer.draw(
            &mut target, &ball, &camera.projection.to_mat(),
            &camera.get_view_transform(), &ball_model
        );

        normal_renderer.draw(
            &mut target, &ball, &camera.projection.to_mat(),
            &camera.get_view_transform(), &ball_model
        );

        target.finish();

        for event in display.poll_events() {
            match event {
                Event::Closed => break 'mainLoop,
                Event::Resized(w, h) => {
                    camera.projection.set_aspect((w as f32)/(h as f32));
                },
                _ => ()
            }
            controller.process_event(&event);
        }
        controller.update(&mut camera, &display);
    }
}


fn create_normal_renderer_program(display: &Display) -> glium::Program {
    let vertex_shader_src = r#"
        #version 330
        in vec3 position;
        in vec3 normal;
        out vec3 g_normal;

        void main() {
            g_normal = normal;
            gl_Position = vec4(position, 1.0);
        }
    "#;

    let geometry_shader_src = r#"
        #version 330
        layout(points) in;

        layout(line_strip, max_vertices = 2) out;

        uniform mat4 MVP;

        in vec3 g_normal[];

        void main() {
            vec4 v0 = gl_in[0].gl_Position;
            gl_Position = MVP * v0;
            EmitVertex();

            vec4 v1 = v0 + vec4(g_normal[0] * 0.5, 0);
            gl_Position = MVP * v1;
            EmitVertex();

            EndPrimitive();
        }
    "#;

    let fragment_shader_src = r#"
        #version 330
        out vec4 frag_color;
        void main() {
            frag_color = vec4(0.0, 0.0, 0.7, 1.0);
        }
    "#;

    glium::Program::from_source(
        display, vertex_shader_src, fragment_shader_src, Some(geometry_shader_src)
    ).unwrap()
}

struct NormalRenderer {
    pub program : glium::Program,
    display    : Display
}

impl NormalRenderer {
    fn new(display: &Display) -> NormalRenderer {
        NormalRenderer {
            program : create_normal_renderer_program(&display),
            display : display.clone()
        }
    }

    fn draw<T>(
        &self, frame: &mut glium::Frame, obj: &T,
        proj: &Mat4<f32>, view: &Iso3<f32>, model: &Iso3<f32>
    ) where T: Renderable {
        let mv = view.prepend_transformation(model);
        let mvp = *proj * nalgebra::to_homogeneous(&mv);

        let uniforms = uniform!(
            MVP: mvp
        );

        let params = glium::DrawParameters {
            depth_test: glium::DepthTest::IfLess,
            depth_write: true,
            .. std::default::Default::default()
        };

        frame.draw(
            &obj.get_vertex_array::<Vertex>(&self.display),
            &NoIndices(PrimitiveType::Points),
            &self.program, &uniforms,
            &params
        ).unwrap();
    }
}

struct LightingRenderer {
    pub program        : glium::Program,
    pub light_position : Vec3<f32>,
    pub diffuse_color  : Vec3<f32>,
    pub specular_color : Vec3<f32>,
    pub shininess      : f32,
    display            : Display
}

impl LightingRenderer {
    fn new(display: &Display) -> LightingRenderer {
        LightingRenderer {
            program        : LightingRenderer::create_shader_program(&display),
            light_position : Vec3::new(0.0, 0.0, 0.0),
            diffuse_color  : Vec3::new(1.0, 1.0, 1.0),
            specular_color : Vec3::new(1.0, 1.0, 1.0),
            shininess      : 128.0,
            display        : display.clone()
        }
    }

    fn draw<T>(
        &self, frame: &mut glium::Frame,
        obj: &T, proj: &Mat4<f32>, view: &Iso3<f32>, model: &Iso3<f32>
    ) where T: Renderable {

        let mv  = view.prepend_transformation(model);
        let mvp = *proj * nalgebra::to_homogeneous(&mv);
        let n   = *mv.to_rot_mat().submat();

        let params = glium::DrawParameters {
            depth_test: glium::DepthTest::IfLess,
            depth_write: true,
            .. std::default::Default::default()
        };

        let uniforms = uniform!(
            MVP            : mvp,
            MV             : nalgebra::to_homogeneous(&mv),
            N              : n,
            light_position : self.light_position,
            diffuse_color  : self.diffuse_color,
            specular_color : self.specular_color,
            shininess      : self.shininess
        );

        match obj.get_indices(&self.display) {
            RenderableIndices::None(primitive) => {
                frame.draw(
                    &obj.get_vertex_array::<Vertex>(&self.display),
                    &NoIndices(primitive),
                    &self.program, &uniforms,
                    &params
                ).unwrap();
            },
            RenderableIndices::Buffer(ref buffer) => {
                frame.draw(
                    &obj.get_vertex_array::<Vertex>(&self.display),
                    buffer,
                    &self.program, &uniforms,
                    &params
                ).unwrap();
            }
        }
    }

    fn create_shader_program(display: &Display) -> glium::Program {
        let vertex_shader_src = r#"
            #version 330
            in vec3 position;
            in vec3 normal;
            smooth out vec3 eye_space_normal;
            smooth out vec3 eye_space_position;

            uniform mat4 MVP;
            uniform mat4 MV;
            uniform mat3 N;

            void main() {
                eye_space_normal = N*normal;
                eye_space_position = (MV*vec4(position, 1)).xyz;
                gl_Position = MVP * vec4(position , 1.0);
            }
        "#;

        let fragment_shader_src = r#"
            #version 330

            smooth in vec3 eye_space_normal;
            smooth in vec3 eye_space_position;
            out vec4 vFragColor;

            uniform vec3 light_position;
            uniform vec3 diffuse_color;
            uniform vec3 specular_color;
            uniform mat4 MV;
            uniform float shininess;

            const vec3 eye_space_camera_pos = vec3(0,0,0);

            void main() {
                vec3 eye_space_light_pos = (MV * vec4(light_position, 1)).xyz;
                vec3 norm                = normalize(eye_space_normal);

                vec3 L = normalize(eye_space_light_pos - eye_space_position);
                vec3 V = normalize(eye_space_camera_pos - eye_space_position);
                vec3 H = normalize(L + V);
                float diffuse  = max(0, dot(norm, L));
                float specular = max(0, pow(dot(eye_space_normal, H), shininess));

                vFragColor = specular*vec4(specular_color, 1) + diffuse*vec4(diffuse_color, 1);

            }
        "#;

        glium::Program::from_source(
            display, vertex_shader_src, fragment_shader_src, None
        ).unwrap()
    }
}


#[derive(Clone, Copy)]
struct Vertex {
    position : [f32; 3],
    normal   : [f32; 3]
}

impl BaseVertex for Vertex {
    fn from_position(x: f32, y: f32, z: f32) -> Vertex {
        Vertex { position : [x, y, z], normal : [0.0, 0.0, 0.0] }
    }
}

impl NormalVertex for Vertex {
    fn set_normal(&mut self, x: f32, y: f32, z: f32) {
        self.normal = [x, y, z];
    }
}

impl RenderableVertex for Vertex {}
