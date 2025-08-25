#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CursorImage {
    pub bytes: &'static str,
    pub offset_x: u8,
    pub offset_y: u8,
}
pub struct CursorImageSystem {
    callback: Box<dyn FnMut(Option<&CursorImage>)>,
    current: Option<CursorImage>,
    published: Option<CursorImage>,
}

impl<T: FnMut(Option<&CursorImage>) + 'static> From<T> for CursorImageSystem {
    fn from(value: T) -> Self {
        CursorImageSystem::new(Box::new(value))
    }
}

impl CursorImageSystem {
    fn new(callback: Box<dyn FnMut(Option<&CursorImage>)>) -> Self {
        Self {
            callback,
            current: None,
            published: None,
        }
    }

    pub fn set(&mut self, current: CursorImage) {
        self.current = Some(current);
    }

    pub fn apply(&mut self) {
        let current = self.current.take();
        if current != self.published {
            if let Some(current) = current {
                (self.callback)(Some(&current));
            } else {
                (self.callback)(None);
            }
            self.published = current;
        }
    }
}
