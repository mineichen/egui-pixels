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

    #[cfg(target_arch = "wasm32")]
    pub fn enable_web(&mut self, canvas: web_sys::HtmlCanvasElement) {
        self.callback = Box::new(move |x: Option<&CursorImage>| {
            if let Some(i) = x {
                log::info!("Add ImageCursor: {i:?}");
                let str = format!(
                    "cursor: url(data:image/png;base64,{}) {} {}, auto;",
                    i.bytes, i.offset_x, i.offset_y
                );
                canvas.set_attribute("style", &str)
            } else {
                log::info!("Remove ImageCursor");
                canvas.remove_attribute("style")
            }
            .ok();
        });
    }

    pub fn set(&mut self, current: CursorImage) {
        self.current = Some(current);
    }

    pub fn apply(&mut self, cursor_in_image: bool) {
        let current = if cursor_in_image {
            self.current.take()
        } else {
            None
        };
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
