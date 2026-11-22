pub struct Application {
    pub server_address: String,
}

impl Application {
    pub fn build(server_address: String) -> Self {
        Self { server_address }
    }
}
