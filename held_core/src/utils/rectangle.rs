use super::position::Position;

#[derive(Debug)]
pub struct Rectangle {
    pub position: Position,
    pub width: usize,
    pub height: usize,
}

impl Rectangle {
    pub fn contains(&self, position: Position) -> bool {
        let end_position = self.position + Position::new(self.height, self.width);
        return position.line >= self.position.line
            && position.line <= end_position.line
            && position.offset >= self.position.offset
            && position.offset <= end_position.offset;
    }
}
