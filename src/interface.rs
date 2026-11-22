use crossterm::event::KeyEvent;
use ratatui::Frame;

use crate::application::Application;

pub struct Interface {
    pub application: Application,
}

impl Interface {
    pub fn new(application: Application) -> Self {
        Self { application }
    }

    pub fn handle_key(&mut self, _key: KeyEvent) {
        todo!("message input is not implemented");
    }

    pub async fn wait_for_update(&mut self) {
        todo!("streamed responses are not implemented");
    }

    pub fn draw(&self, _frame: &mut Frame<'_>) {
        todo!("the chat interface is not implemented");
    }
}
