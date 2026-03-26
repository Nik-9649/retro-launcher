use std::time::{Duration, Instant};

/// Types of toast notifications
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastType {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastType {
    /// Get the Unicode icon for this toast type
    pub fn icon(&self) -> &'static str {
        match self {
            ToastType::Info => "ℹ",
            ToastType::Success => "✓",
            ToastType::Warning => "⚠",
            ToastType::Error => "✗",
        }
    }
}

/// Animation state for slide-in/out effects
#[derive(Debug, Clone, Copy)]
pub enum AnimationState {
    /// Sliding in from the right, progress 0.0 to 1.0
    SlidingIn { progress: f32 },
    /// Fully visible and stationary
    Visible,
    /// Sliding out to the right, progress 0.0 to 1.0
    SlidingOut { progress: f32 },
}

/// Individual toast notification
#[derive(Debug, Clone)]
pub struct Toast {
    pub id: u64,
    pub toast_type: ToastType,
    pub message: String,
    pub created_at: Instant,
    pub duration: Duration,
    pub animation_state: AnimationState,
    pub animation_started: Instant,
}

impl Toast {
    /// Create a new toast with the given type and message
    fn new(id: u64, toast_type: ToastType, message: String, duration: Duration) -> Self {
        let now = Instant::now();
        Self {
            id,
            toast_type,
            message,
            created_at: now,
            duration,
            animation_state: AnimationState::SlidingIn { progress: 0.0 },
            animation_started: now,
        }
    }

    /// Check if this toast is currently visible (not fully dismissed)
    pub fn is_visible(&self) -> bool {
        !matches!(self.animation_state, AnimationState::SlidingOut { progress: 1.0 })
    }

    /// Start the slide-out animation
    pub fn dismiss(&mut self) {
        self.animation_state = AnimationState::SlidingOut { progress: 0.0 };
        self.animation_started = Instant::now();
    }
}

/// Manages a queue of toast notifications with animations
#[derive(Debug)]
pub struct ToastManager {
    toasts: Vec<Toast>,
    next_id: u64,
    max_visible: usize,
    default_duration: Duration,
    animation_duration: Duration,
}

impl ToastManager {
    /// Default duration for toast visibility (4 seconds)
    pub const DEFAULT_DURATION: Duration = Duration::from_secs(4);
    /// Default animation duration (200ms for slide-in, 150ms for slide-out)
    pub const DEFAULT_ANIMATION_DURATION: Duration = Duration::from_millis(200);
    /// Default maximum number of visible toasts
    pub const DEFAULT_MAX_VISIBLE: usize = 5;

    /// Create a new ToastManager with default settings
    pub fn new() -> Self {
        Self {
            toasts: Vec::new(),
            next_id: 1,
            max_visible: Self::DEFAULT_MAX_VISIBLE,
            default_duration: Self::DEFAULT_DURATION,
            animation_duration: Self::DEFAULT_ANIMATION_DURATION,
        }
    }

    /// Add a new toast with the given type and message
    pub fn add(&mut self, toast_type: ToastType, message: impl Into<String>) {
        let message = message.into();

        // Deduplicate: don't add identical messages to what's already showing
        if self.toasts.iter().any(|t| {
            t.toast_type == toast_type
                && t.message == message
                && matches!(t.animation_state, AnimationState::SlidingIn { .. } | AnimationState::Visible)
        }) {
            return;
        }

        let toast = Toast::new(self.next_id, toast_type, message, self.default_duration);
        self.next_id += 1;

        self.toasts.push(toast);

        // Enforce max visible limit by dismissing oldest toasts
        let visible_count = self
            .toasts
            .iter()
            .filter(|t| matches!(t.animation_state, AnimationState::SlidingIn { .. } | AnimationState::Visible))
            .count();

        if visible_count > self.max_visible {
            // Find the oldest toast (first one that's not already dismissing) and dismiss it
            if let Some(oldest) = self
                .toasts
                .iter_mut()
                .find(|t| matches!(t.animation_state, AnimationState::SlidingIn { .. } | AnimationState::Visible))
            {
                oldest.dismiss();
            }
        }
    }

    /// Add an info toast
    pub fn info(&mut self, message: impl Into<String>) {
        self.add(ToastType::Info, message);
    }

    /// Add a success toast
    pub fn success(&mut self, message: impl Into<String>) {
        self.add(ToastType::Success, message);
    }

    /// Add a warning toast
    pub fn warning(&mut self, message: impl Into<String>) {
        self.add(ToastType::Warning, message);
    }

    /// Add an error toast
    pub fn error(&mut self, message: impl Into<String>) {
        self.add(ToastType::Error, message);
    }

    /// Get all toasts for rendering (including animating ones)
    pub fn toasts(&self) -> &[Toast] {
        &self.toasts
    }

    /// Get the number of active toasts
    pub fn len(&self) -> usize {
        self.toasts.len()
    }

    /// Check if there are no active toasts
    pub fn is_empty(&self) -> bool {
        self.toasts.is_empty()
    }

    /// Update animation states and remove expired toasts
    pub fn tick(&mut self) {
        let now = Instant::now();

        for toast in &mut self.toasts {
            match toast.animation_state {
                AnimationState::SlidingIn { .. } => {
                    let elapsed = now.duration_since(toast.animation_started).as_millis() as f32;
                    let duration = self.animation_duration.as_millis() as f32;
                    let progress = (elapsed / duration).min(1.0);

                    if progress >= 1.0 {
                        toast.animation_state = AnimationState::Visible;
                    } else {
                        toast.animation_state = AnimationState::SlidingIn { progress };
                    }
                }
                AnimationState::Visible => {
                    let elapsed = now.duration_since(toast.created_at);
                    if elapsed >= toast.duration {
                        toast.animation_state = AnimationState::SlidingOut { progress: 0.0 };
                        toast.animation_started = now;
                    }
                }
                AnimationState::SlidingOut { .. } => {
                    // Use a shorter duration for slide-out (150ms)
                    let slide_out_duration = self.animation_duration.mul_f32(0.75);
                    let elapsed = now.duration_since(toast.animation_started).as_millis() as f32;
                    let duration = slide_out_duration.as_millis() as f32;
                    let progress = (elapsed / duration).min(1.0);

                    if progress >= 1.0 {
                        toast.animation_state = AnimationState::SlidingOut { progress: 1.0 };
                    } else {
                        toast.animation_state = AnimationState::SlidingOut { progress };
                    }
                }
            }
        }

        // Remove fully animated-out toasts
        self.toasts
            .retain(|toast| !matches!(toast.animation_state, AnimationState::SlidingOut { progress: 1.0 }));
    }

    /// Dismiss the most recent visible toast
    pub fn dismiss_latest(&mut self) {
        // Find the most recent toast that's still visible and dismiss it
        if let Some(toast) = self
            .toasts
            .iter_mut()
            .rev()
            .find(|t| matches!(t.animation_state, AnimationState::SlidingIn { .. } | AnimationState::Visible))
        {
            toast.dismiss();
        }
    }

    /// Dismiss all visible toasts
    pub fn dismiss_all(&mut self) {
        for toast in &mut self.toasts {
            if matches!(toast.animation_state, AnimationState::SlidingIn { .. } | AnimationState::Visible) {
                toast.dismiss();
            }
        }
    }

    /// Clear all toasts immediately without animation
    pub fn clear(&mut self) {
        self.toasts.clear();
    }
}

impl Default for ToastManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_toast() {
        let mut manager = ToastManager::new();
        manager.info("Test message");
        assert_eq!(manager.len(), 1);
    }

    #[test]
    fn test_deduplication() {
        let mut manager = ToastManager::new();
        manager.info("Duplicate");
        manager.info("Duplicate");
        assert_eq!(manager.len(), 1);
    }

    #[test]
    fn test_dismiss_latest() {
        let mut manager = ToastManager::new();
        manager.info("First");
        manager.info("Second");
        manager.dismiss_latest();

        // Second toast should be sliding out
        let toasts = manager.toasts();
        assert!(matches!(toasts[1].animation_state, AnimationState::SlidingOut { .. }));
    }

    #[test]
    fn test_max_visible_limit() {
        let mut manager = ToastManager::new();
        manager.max_visible = 2;

        manager.info("First");
        manager.info("Second");
        manager.info("Third"); // Should cause first to start dismissing

        let visible_count = manager
            .toasts
            .iter()
            .filter(|t| matches!(t.animation_state, AnimationState::SlidingIn { .. } | AnimationState::Visible))
            .count();

        assert_eq!(visible_count, 2);
    }

    #[test]
    fn test_animation_progress() {
        let mut manager = ToastManager::new();
        manager.info("Test");

        // Initially sliding in
        assert!(matches!(manager.toasts()[0].animation_state, AnimationState::SlidingIn { .. }));

        // Simulate time passing
        std::thread::sleep(Duration::from_millis(250));
        manager.tick();

        // Should be visible now
        assert!(matches!(manager.toasts()[0].animation_state, AnimationState::Visible));
    }
}
