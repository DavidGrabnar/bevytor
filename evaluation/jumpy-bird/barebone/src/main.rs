use bevy::prelude::*;
use bevy::render::camera::ScalingMode;
use bevy::window::PrimaryWindow;
use rand::Rng;

const SCALE: f32 = 4.0;
const HEIGHT: f32 = 20.0;

const BIRD_WIDTH: f32 = 2.0;
const BIRD_HEIGHT: f32 = 2.0;

const HEIGHT_FLOOR: f32 = 2.0;

const INITIAL_BIRD_OFFSET: f32 = -10.0;
const INITIAL_PILLAR_OFFSET: f32 = 10.0;

const NUM_PILLARS: i32 = 20;
const WIDTH_PILLARS_CENTER: f32 = 10.0;
const WIDTH_PILLAR: f32 = 3.0;
const HEIGHT_FULL_PILLAR: f32 = HEIGHT - HEIGHT_FLOOR;
const HEIGHT_GAP_PILLAR: f32 = BIRD_HEIGHT * 3.0;
const HEIGHT_MIN_PILLAR: f32 = HEIGHT_GAP_PILLAR;
const HEIGHT_MAX_PILLAR: f32 = HEIGHT_FULL_PILLAR - HEIGHT_GAP_PILLAR;

const FLOOR: f32 = HEIGHT_FLOOR - HEIGHT_FULL_PILLAR / 2.0;
const CEILING: f32 = HEIGHT_FLOOR + HEIGHT_FULL_PILLAR / 2.0 - BIRD_HEIGHT;

const VELOCITY: f32 = 4.0;

const GRAVITY: f32 = -12.0;
const JUMP_FORCE: f32 = 7.0;

const NORMAL_BUTTON: Color = Color::rgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::rgb(0.25, 0.25, 0.25);
const PRESSED_BUTTON: Color = Color::rgb(0.35, 0.75, 0.35);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_state::<AppState>()
        .add_systems(Startup, init_scene)
        // menu
        .add_systems(OnEnter(AppState::Menu), setup_menu)
        .add_systems(Update, menu.run_if(in_state(AppState::Menu)))
        .add_systems(OnExit(AppState::Menu), cleanup_menu)
        // in game
        .add_systems(OnEnter(AppState::InGame), setup_game)
        .add_systems(
            Update,
            (move_pillars, move_bird, check_end).run_if(in_state(AppState::InGame)),
        )
        .run();
}

enum PillarPos {
    Bottom,
    Top,
}

#[derive(Component)]
struct Pillar(PillarPos, i32, f32); // pos, iteration, height

#[derive(Component)]
struct Bird(f32); // vertical velocity

#[derive(Component)]
struct ScoreBoard(f32); // offset from start

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
enum AppState {
    #[default]
    Menu,
    InGame,
}

#[derive(Resource)]
struct MenuData {
    button_entity: Entity,
}

fn init_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // camera
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 0.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        projection: OrthographicProjection {
            scale: SCALE,
            scaling_mode: ScalingMode::FixedVertical(HEIGHT / SCALE),
            ..default()
        }
        .into(),
        ..default()
    });
    // ground
    commands.spawn(PbrBundle {
        mesh: meshes.add(shape::Box::new(40.0, HEIGHT_FLOOR, 1.0).into()),
        material: materials.add(Color::rgb(0.3, 0.5, 0.3).into()),
        transform: Transform::from_xyz(0.0, -9.0, 0.0),
        ..default()
    });

    // pillars
    let mut rng = rand::thread_rng();
    for i in 0..NUM_PILLARS {
        let height_bottom = rng.gen_range(HEIGHT_MIN_PILLAR..=HEIGHT_MAX_PILLAR);
        let center_offset_bottom = (-HEIGHT_FULL_PILLAR + HEIGHT_FLOOR + height_bottom) * 0.5;
        spawn_pillar(
            &mut commands,
            &mut meshes,
            &mut materials,
            i,
            height_bottom,
            PillarPos::Bottom,
            center_offset_bottom,
        );

        let height_top = HEIGHT_FULL_PILLAR - HEIGHT_GAP_PILLAR - height_bottom;
        let center_offset_top = (HEIGHT_FULL_PILLAR + HEIGHT_FLOOR - height_top) * 0.5;
        spawn_pillar(
            &mut commands,
            &mut meshes,
            &mut materials,
            i,
            height_top,
            PillarPos::Top,
            center_offset_top,
        );
    }

    // bird
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(shape::Box::new(BIRD_WIDTH, BIRD_HEIGHT, 1.0).into()),
            material: materials.add(Color::rgb(0.8, 0.2, 0.3).into()),
            transform: Transform::from_xyz(INITIAL_BIRD_OFFSET, 0.0, 2.0),
            ..default()
        })
        .insert(Bird(JUMP_FORCE));

    commands
        .spawn(NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.),
                justify_content: JustifyContent::Start,
                align_items: AlignItems::Start,
                ..default()
            },
            ..default()
        })
        .with_children(|parent| {
            parent
                .spawn(TextBundle::from_section(
                    "0",
                    TextStyle {
                        font_size: 40.0,
                        ..default()
                    },
                ))
                .insert(ScoreBoard(0.0));
        });
}

fn setup_menu(mut commands: Commands) {
    let button_entity = commands
        .spawn(NodeBundle {
            style: Style {
                // center button
                width: Val::Percent(100.),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ..default()
        })
        .with_children(|parent| {
            parent
                .spawn(ButtonBundle {
                    style: Style {
                        width: Val::Px(150.),
                        height: Val::Px(65.),
                        // horizontally center child text
                        justify_content: JustifyContent::Center,
                        // vertically center child text
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    background_color: NORMAL_BUTTON.into(),
                    ..default()
                })
                .with_children(|parent| {
                    parent.spawn(TextBundle::from_section(
                        "Play",
                        TextStyle {
                            font_size: 40.0,
                            color: Color::rgb(0.9, 0.9, 0.9),
                            ..default()
                        },
                    ));
                });
        })
        .id();
    commands.insert_resource(MenuData { button_entity });
}

fn menu(
    mut next_state: ResMut<NextState<AppState>>,
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = PRESSED_BUTTON.into();
                next_state.set(AppState::InGame);
            }
            Interaction::Hovered => {
                *color = HOVERED_BUTTON.into();
            }
            Interaction::None => {
                *color = NORMAL_BUTTON.into();
            }
        }
    }
}

fn cleanup_menu(mut commands: Commands, menu_data: Res<MenuData>) {
    commands.entity(menu_data.button_entity).despawn_recursive();
}

fn setup_game(
    mut bird_query: Query<(&mut Transform, &mut Bird), (With<Bird>, Without<Pillar>)>,
    mut pillar_query: Query<(&mut Transform, &Pillar), With<Pillar>>,
    mut scoreboard_query: Query<&mut ScoreBoard>,
) {
    let (mut bird_transform, mut bird) = bird_query.single_mut();
    bird_transform.translation.x = INITIAL_BIRD_OFFSET;
    bird_transform.translation.y = 0.0;
    bird.0 = JUMP_FORCE;

    for (mut pillar_transform, pillar) in pillar_query.iter_mut() {
        pillar_transform.translation.x =
            pillar.1 as f32 * WIDTH_PILLARS_CENTER + INITIAL_PILLAR_OFFSET;
    }

    let mut scoreboard = scoreboard_query.single_mut();
    scoreboard.0 = 0.0;
}

fn move_pillars(
    mut query: Query<&mut Transform, With<Pillar>>,
    mut scoreboard_query: Query<(&mut Text, &mut ScoreBoard)>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    time: Res<Time>,
) {
    let window = primary_window.single();
    let ratio = window.resolution.width() / window.resolution.height();
    let max_horizontal = ratio * HEIGHT * 0.5 - WIDTH_PILLAR * 0.5;
    for mut transform in query.iter_mut() {
        transform.translation.x -= VELOCITY * time.delta_seconds();
        if transform.translation.x < -max_horizontal {
            transform.translation.x += NUM_PILLARS as f32 * WIDTH_PILLARS_CENTER;
        }
    }
    let (mut text, mut scoreboard) = scoreboard_query.single_mut();
    scoreboard.0 += VELOCITY * time.delta_seconds();

    text.sections.get_mut(0).unwrap().value = (scoreboard.0 as i32).to_string();
}

fn spawn_pillar(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    iteration: i32,
    height: f32,
    position: PillarPos,
    center_offset: f32,
) {
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(shape::Box::new(WIDTH_PILLAR, height, 1.0).into()),
            material: materials.add(Color::rgb(0.8, 0.7, 0.6).into()),
            transform: Transform::from_xyz(
                iteration as f32 * WIDTH_PILLARS_CENTER + INITIAL_PILLAR_OFFSET,
                center_offset,
                1.0,
            ),
            ..default()
        })
        .insert(Pillar(position, iteration, height));
}

fn move_bird(
    mut query: Query<(&mut Transform, &mut Bird), With<Bird>>,
    keys: Res<Input<KeyCode>>,
    time: Res<Time>,
) {
    let (mut transform, mut bird) = query.single_mut();
    if keys.just_pressed(KeyCode::Space) {
        bird.0 = JUMP_FORCE;
    } else if transform.translation.y <= FLOOR {
        transform.translation.y = FLOOR;
    } else if transform.translation.y >= CEILING {
        transform.translation.y = CEILING;
    }
    bird.0 += GRAVITY * time.delta_seconds();
    if bird.0 >= JUMP_FORCE {
        bird.0 = JUMP_FORCE;
    } else if bird.0 <= GRAVITY {
        bird.0 = GRAVITY;
    }
    transform.translation.y += bird.0 * time.delta_seconds();
}

fn check_end(
    mut next_state: ResMut<NextState<AppState>>,
    bird_query: Query<&Transform, With<Bird>>,
    pillar_query: Query<(&Transform, &Pillar), With<Pillar>>,
    keys: Res<Input<KeyCode>>,
) {
    if keys.just_pressed(KeyCode::A) {
        next_state.set(AppState::Menu);
    }

    let bird_transform = bird_query.single();
    if bird_transform.translation.y <= FLOOR || bird_transform.translation.y >= CEILING {
        next_state.set(AppState::Menu);
    }

    for (transform, pillar) in pillar_query.iter() {
        let collision_x = bird_transform.translation.x + BIRD_WIDTH / 2.0
            >= transform.translation.x - WIDTH_PILLAR / 2.0
            && transform.translation.x + WIDTH_PILLAR / 2.0
                >= bird_transform.translation.x - BIRD_WIDTH / 2.0;
        let collision_y = bird_transform.translation.y + BIRD_HEIGHT / 2.0
            >= transform.translation.y - pillar.2 / 2.0
            && transform.translation.y + pillar.2 / 2.0
                >= bird_transform.translation.y - BIRD_HEIGHT / 2.0;
        // collision only if on both axes
        if collision_x && collision_y {
            next_state.set(AppState::Menu);
            break;
        }
    }
}
