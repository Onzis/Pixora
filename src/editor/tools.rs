#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    Select,
    Pencil,
    Marker,
    Arrow,
    Line,
    Rectangle,
    Ellipse,
    Triangle,
    Star,
    Diamond,
    Text,
    Blur,
    Pixelate,
    Spotlight,
    NumberBadge,
    Sticker,
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
            ToolKind::Triangle => "Triangle",
            ToolKind::Star => "Star",
            ToolKind::Diamond => "Diamond",
            ToolKind::Text => "Text",
            ToolKind::Blur => "Blur",
            ToolKind::Pixelate => "Pixelate",
            ToolKind::Spotlight => "Spotlight",
            ToolKind::NumberBadge => "Step Badge",
            ToolKind::Sticker => "Sticker",
            ToolKind::Eyedropper => "Color Picker",
        }
    }

    pub fn display_name(&self, is_ru: bool) -> &'static str {
        match self {
            ToolKind::Select => if is_ru { "Выделение" } else { "Select" },
            ToolKind::Pencil => if is_ru { "Карандаш" } else { "Pencil" },
            ToolKind::Marker => if is_ru { "Маркер" } else { "Highlighter" },
            ToolKind::Arrow => if is_ru { "Стрелка" } else { "Arrow" },
            ToolKind::Line => if is_ru { "Линия" } else { "Line" },
            ToolKind::Rectangle => if is_ru { "Прямоугольник" } else { "Rectangle" },
            ToolKind::Ellipse => if is_ru { "Эллипс" } else { "Ellipse" },
            ToolKind::Triangle => if is_ru { "Треугольник" } else { "Triangle" },
            ToolKind::Star => if is_ru { "Звезда" } else { "Star" },
            ToolKind::Diamond => if is_ru { "Ромб" } else { "Diamond" },
            ToolKind::Text => if is_ru { "Текст" } else { "Text" },
            ToolKind::Blur => if is_ru { "Размытие" } else { "Blur" },
            ToolKind::Pixelate => if is_ru { "Пикселизация" } else { "Pixelate" },
            ToolKind::Spotlight => if is_ru { "Фокус / Затемнение" } else { "Spotlight" },
            ToolKind::NumberBadge => if is_ru { "Шаги / Нумерация" } else { "Step Badge" },
            ToolKind::Sticker => if is_ru { "Стикеры и значки" } else { "Stickers" },
            ToolKind::Eyedropper => if is_ru { "Пипетка" } else { "Color Picker" },
        }
    }

    pub fn icon_path(&self) -> &'static str {
        match self {
            ToolKind::Select => "icons/select.svg",
            ToolKind::Pencil => "icons/pencil.svg",
            ToolKind::Marker => "icons/marker.svg",
            ToolKind::Arrow => "icons/arrow.svg",
            ToolKind::Line => "icons/line.svg",
            ToolKind::Rectangle => "icons/rectangle.svg",
            ToolKind::Ellipse => "icons/circle.svg",
            ToolKind::Triangle => "icons/triangle.svg",
            ToolKind::Star => "icons/star.svg",
            ToolKind::Diamond => "icons/diamond.svg",
            ToolKind::Text => "icons/text.svg",
            ToolKind::Blur => "icons/blur.svg",
            ToolKind::Pixelate => "icons/pixelate.svg",
            ToolKind::Spotlight => "icons/spotlight.svg",
            ToolKind::NumberBadge => "icons/step.svg",
            ToolKind::Sticker => "icons/sticker.svg",
            ToolKind::Eyedropper => "icons/eyedropper.svg",
        }
    }

    pub fn is_shape(&self) -> bool {
        matches!(
            self,
            ToolKind::Rectangle
                | ToolKind::Ellipse
                | ToolKind::Triangle
                | ToolKind::Star
                | ToolKind::Diamond
                | ToolKind::Arrow
                | ToolKind::Line
        )
    }
}
