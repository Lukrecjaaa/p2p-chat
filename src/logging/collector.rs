use super::LogBuffer;
use crate::ui::LogEntry;
use chrono::Utc;
use std::sync::Arc;
use tracing::{Event, Subscriber};
use tracing_subscriber::{
    layer::{Context, SubscriberExt},
    registry::LookupSpan,
    Layer,
};

pub struct TUILogCollector {
    buffer: Arc<LogBuffer>,
}

impl TUILogCollector {
    pub fn new(buffer: Arc<LogBuffer>) -> Self {
        Self { buffer }
    }

    pub fn init_subscriber(
        buffer: Arc<LogBuffer>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let collector = TUILogCollector::new(buffer);

        // Create a layered subscriber with ONLY TUI output (no console output)
        let subscriber = tracing_subscriber::registry().with(collector);

        tracing::subscriber::set_global_default(subscriber)?;
        Ok(())
    }
}

impl<S> Layer<S> for TUILogCollector
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();

        // Extract the message from the event
        let mut message = String::new();
        let mut visitor = MessageVisitor(&mut message);
        event.record(&mut visitor);

        // Get the module path
        let target = metadata.target();
        let module = if let Some(module_path) = metadata.module_path() {
            // Extract the last component for cleaner display
            module_path
                .split("::")
                .last()
                .unwrap_or(module_path)
                .to_string()
        } else {
            target.to_string()
        };

        // Create log entry
        let entry = LogEntry {
            timestamp: Utc::now(),
            level: *metadata.level(),
            module,
            message,
        };

        // Add to buffer
        self.buffer.add_entry(entry);
    }
}

struct MessageVisitor<'a>(&'a mut String);

impl<'a> tracing::field::Visit for MessageVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            *self.0 = format!("{:?}", value);
        } else {
            if !self.0.is_empty() {
                self.0.push(' ');
            }
            self.0.push_str(&format!("{}={:?}", field.name(), value));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            *self.0 = value.to_string();
        } else {
            if !self.0.is_empty() {
                self.0.push(' ');
            }
            self.0.push_str(&format!("{}={}", field.name(), value));
        }
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        if !self.0.is_empty() {
            self.0.push(' ');
        }
        self.0.push_str(&format!("{}={}", field.name(), value));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if !self.0.is_empty() {
            self.0.push(' ');
        }
        self.0.push_str(&format!("{}={}", field.name(), value));
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        if !self.0.is_empty() {
            self.0.push(' ');
        }
        self.0.push_str(&format!("{}={}", field.name(), value));
    }
}
