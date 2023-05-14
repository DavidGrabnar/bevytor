use bevy::prelude::*;

#[no_mangle]
pub fn test_hot_system(mut query: Query<&mut Transform>) {
    for mut tfm in &mut query {
        tfm.translation.x += 0.001;
    }
}
