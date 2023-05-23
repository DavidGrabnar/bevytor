use std::collections::linked_list::Iter;
use bevy::prelude::*;
use std::collections::LinkedList;

#[derive(Resource, Default)]
pub struct LogBuffer(LinkedList<String>);

const LOG_SIZE: usize = 100;

impl LogBuffer {
    pub fn write(&mut self, content: String) {
        if self.0.len() == LOG_SIZE {
            self.0.pop_front();
        }
        self.0.push_back(content);
    }
    
    pub fn iter(&self) -> Iter<String> {
        self.0.iter()
    }
}
