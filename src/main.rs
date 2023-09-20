use bevy::prelude::*;
use bevy::window::PrimaryWindow;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup_template_scene)
        .add_systems(Update, save_scene)
        .run();
}

fn save_scene(world: &mut World) {
    // just to be sure
    std::thread::sleep(std::time::Duration::from_secs(2));

    let type_registry = world.resource::<AppTypeRegistry>();

    let mut builder = DynamicSceneBuilder::from_world(world);
    builder.deny_all_resources().extract_resources();
    builder
        .allow_all()
        .deny::<ComputedVisibility>()
        .deny::<Window>()
        .deny::<PrimaryWindow>()
        .extract_entities(world.iter_entities().map(|r| r.id()))
        .remove_empty_entities();

    let scene = builder.build();
    println!("{} {}", scene.entities.len(), scene.resources.len());
    let scene_serialized = scene.serialize_ron(type_registry).unwrap();

    std::fs::write(std::path::Path::new("./initial.ron"), scene_serialized).unwrap();

    println!("saved");
}

fn setup_template_scene(
    world: &mut World,
    //commands: &mut Commands,
    //mut materials: ResMut<Assets<StandardMaterial>>,
    //mut meshes: ResMut<Assets<Mesh>>,
) {
    // camera
    world.spawn(Camera3dBundle {
        transform: Transform::from_xyz(5.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

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

    // child cube
    let bundle = setup_template_pbr_bundle(
        world,
        Mesh::from(shape::Cube { size: 2.0 }),
        Color::rgb(0.8, 0.7, 0.6),
        Transform::from_xyz(0.0, 1.0, 0.0),
    );
    let cube = world.spawn(bundle).id();

    // parent cube
    let bundle = setup_template_pbr_bundle(
        world,
        Mesh::from(shape::Cube { size: 1.0 }),
        Color::rgb(0.6, 0.7, 0.8),
        Transform::from_xyz(0.0, 1.5, 0.0),
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
