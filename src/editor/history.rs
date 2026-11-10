use crate::editor::annotations::Annotation;

#[derive(Clone, Debug, Default)]
pub struct EditHistory {
    pub annotations: Vec<Annotation>,
    undo_stack: Vec<Vec<Annotation>>,
    redo_stack: Vec<Vec<Annotation>>,
}

impl EditHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&mut self) {
        self.undo_stack.push(self.annotations.clone());
        self.redo_stack.clear();
    }

    pub fn push_annotation(&mut self, annotation: Annotation) {
        self.snapshot();
        self.annotations.push(annotation);
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(self.annotations.clone());
            self.annotations = prev;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.annotations.clone());
            self.annotations = next;
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        if !self.annotations.is_empty() {
            self.snapshot();
            self.annotations.clear();
        }
    }
}
