#[derive(Debug)]
pub struct Song {
    pub name: String,
    pub id: String,
}

impl Song {
    pub fn new(name: String, id: String) -> Song {
        Song {
            name,
            id,
        }
    }
}