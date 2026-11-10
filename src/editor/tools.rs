#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    Select,
    Pencil,
    Marker,
    Arrow,
    Line,
    Rectangle,
    Ellipse,
    Text,
    Blur,
    Pixelate,
    NumberBadge,
    Eyedropper,
}

impl ToolKind {
    pub fn name(&self) -> &'static str {
        match self {
            ToolKind::Select => "Select",
            ToolKind::Pencil => "Pencil",
            ToolKind::Marker => "Highlighter",
            ToolKind::Arrow => "Arrow",
            ToolKind::Line => "Line",
            ToolKind::Rectangle => "Rectangle",
            ToolKind::Ellipse => "Ellipse",
            ToolKind::Text => "Text",
            ToolKind::Blur => "Blur",
            ToolKind::Pixelate => "Pixelate",
            ToolKind::NumberBadge => "Step Badge",
            ToolKind::Eyedropper => "Color Picker",
        }
    }
}
