use bevy::prelude::*;
use bevy::scene::serialize_ron;
// use bevytor_core::tree::Node;
// use bevytor_script::{CreateScript, Script};
// use libloading::{Library, Symbol};

/*fn main() {
    let mut loaded_libraries: Vec<Library> = vec![];
    let mut loaded_plugins: Vec<Box<dyn Script>> = vec![];

    unsafe {
        let lib =
            Library::new("C:\\Users\\grabn\\Documents\\Faks\\bevytor\\cargo\\test_plugin\\target\\debug\\test_plugin.dll").unwrap();

        // We need to keep the library around otherwise our plugin's vtable will
        // point to garbage. We do this little dance to make sure the library
        // doesn't end up getting moved.
        loaded_libraries.push(lib);

        let lib = loaded_libraries.last().unwrap();

        let constructor: Symbol<CreateScript> = lib.get(b"create_plugin").unwrap();
        let boxed_raw = constructor();

        let plugin = Box::from_raw(boxed_raw);
        println!("Loaded plugin: {} {}", plugin.name(), plugin.sum(1, 2));
        let mut w = World::new();
        w.spawn(());
        plugin.run(&mut w);
        loaded_plugins.push(plugin);
        println!("Entities: {}", w.entities().len());
    }
}*/

fn main() {
    // let test = Node::default();
    // println!("test {:?}", test);
    println!("root {:?}", std::any::TypeId::of::<Transform>());
    App::new()
        .add_plugins(DefaultPlugins)
        .add_startup_system(setup_template_scene)
        .add_startup_system(save_scene)
        //.add_system(test)
        //.register_type::<CursorIcon>()
        //.register_type::<bevy::window::CursorGrabMode>()
        //.register_type::<bevy::window::CompositeAlphaMode>()
        //.register_type::<Option<bevy::math::DVec2>>()
        //.register_type::<Option<bool>>()
        //.register_type::<Option<f64>>()
        //.register_type::<bevy::window::WindowLevel>()
        .register_type::<Rect>()
        .register_type_data::<Rect, ReflectSerialize>()
        .register_type_data::<Rect, ReflectDeserialize>()
        .run();
}

fn setup_scene(mut commands: Commands) {
    //commands.spawn(Camera3dBundle::default());
}

fn save_scene(world: &mut World) {
    // just to be sure
    std::thread::sleep(std::time::Duration::from_secs(2));

    world.resource_scope(|world, type_registry: Mut<AppTypeRegistry>| {
        setup_template_scene(world);
        //let type_registry = type_registry.read();
        let scene = DynamicScene::from_world(world, &type_registry);
        let scene_serialized = scene.serialize_ron(&type_registry).unwrap();

        std::fs::write(std::path::Path::new("./help-plz.ron"), scene_serialized).unwrap();
    });

    println!("saved");
}

fn setup_template_scene(
    world: &mut World,
    //commands: &mut Commands,
    //mut materials: ResMut<Assets<StandardMaterial>>,
    //mut meshes: ResMut<Assets<Mesh>>,
) {
    // set up the camera
    let mut camera = Camera3dBundle::default();
    let mut proj = OrthographicProjection::default();
    proj.area = Rect::new(-1.0, -1.0, 1.0, 1.0);
    proj.scale = 0.01;
    //camera.projection.get_projection_matrix().mul_scalar(3.0);
    camera.transform = Transform::from_xyz(5.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y);
    camera.projection = Projection::Orthographic(proj);
    println!(
        "{}",
        serialize_ron(Rect::new(-1.0, -1.0, 1.0, 1.0)).unwrap()
    );
    // camera
    world.spawn(camera);

    // plane
    let bundle = setup_template_pbr_bundle(
        world,
        Mesh::from(shape::Plane {
            size: 5.0,
            subdivisions: 0,
        }),
        Color::rgb(0.3, 0.5, 0.3),
        Transform::default(),
    );
    world.spawn(bundle);
    // cube
    let bundle = setup_template_pbr_bundle(
        world,
        Mesh::from(shape::Cube { size: 1.0 }),
        Color::rgb(0.8, 0.7, 0.6),
        Transform::from_xyz(0.0, 0.5, 0.0),
    );
    let cube = world.spawn(bundle).id();
    // child cube
    let bundle = setup_template_pbr_bundle(
        world,
        Mesh::from(shape::Cube { size: 1.0 }),
        Color::rgb(0.6, 0.7, 0.8),
        Transform::from_xyz(0.0, 1.0, 0.0),
    );
    world.spawn(bundle).push_children(&[cube]);
    // light
    world.spawn(PointLightBundle {
        transform: Transform::from_xyz(3.0, 8.0, 5.0),
        ..Default::default()
    });
}

fn setup_template_pbr_bundle(
    //materials: &mut Assets<StandardMaterial>,
    //meshes: &mut Assets<Mesh>,
    world: &mut World,
    mesh: Mesh,
    color: Color,
    transform: Transform,
) -> PbrBundle {
    PbrBundle {
        mesh: world.resource_mut::<Assets<Mesh>>().add(mesh),
        material: world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(color.into()),
        transform,
        ..Default::default()
    }
}

/*fn test(mut query: Query<(Entity, &mut Transform)>) {
    for (entity, mut transform) in &mut query {
        println!("Moving {:?} {}", entity, transform.translation.x);
        transform.translation.x += 1.0;
    }
    println!("test {:?}", query);
}*/
