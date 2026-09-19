//! Service lifecycle and dispatch helpers.
//!
//! A service owns its state and handles one ABI message at a time. The
//! registry is a convenience for hosted applications; the `Service` trait and
//! [`EchoService`] are also available without the `std` feature.

use crate::error::{Error, ErrorCode, Result};
use crate::ipc::{message_type, Message};

/// A Rootware userspace service.
pub trait Service {
    /// Stable service name used during registration and dispatch.
    fn name(&self) -> &str;

    /// Initializes service state before it accepts messages.
    fn init(&mut self) -> Result<()>;

    /// Handles one request and optionally returns a response message.
    fn handle(&mut self, request: &Message) -> Result<Option<Message>>;

    /// Stops the service and releases service-owned state.
    fn stop(&mut self) {}
}

/// Minimal service that echoes requests back to their sender.
#[derive(Debug, Default)]
pub struct EchoService {
    initialized: bool,
}

impl EchoService {
    /// Creates an uninitialized echo service.
    pub const fn new() -> Self {
        Self { initialized: false }
    }
}

impl Service for EchoService {
    fn name(&self) -> &str {
        "echo"
    }

    fn init(&mut self) -> Result<()> {
        self.initialized = true;
        Ok(())
    }

    fn handle(&mut self, request: &Message) -> Result<Option<Message>> {
        if !self.initialized || request.sender == 0 || request.receiver == 0 {
            return Err(Error::new(ErrorCode::InvalidArgument));
        }
        Ok(Some(Message::new(
            request.receiver,
            request.sender,
            message_type::RESPONSE,
            request.capability,
            request.payload,
        )))
    }

    fn stop(&mut self) {
        self.initialized = false;
    }
}

/// Lifecycle state of a registered service.
#[cfg(feature = "std")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceState {
    /// The registry has not initialized its services.
    Stopped,
    /// All registered services have been initialized.
    Running,
}

/// Service registry and lifecycle manager.
#[cfg(feature = "std")]
pub struct ServiceRegistry {
    services: std::vec::Vec<std::boxed::Box<dyn Service>>,
    state: ServiceState,
}

#[cfg(feature = "std")]
impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "std")]
impl ServiceRegistry {
    /// Creates an empty, stopped registry.
    pub fn new() -> Self {
        Self {
            services: std::vec::Vec::new(),
            state: ServiceState::Stopped,
        }
    }

    /// Registers a service before the registry is started.
    pub fn register(&mut self, service: std::boxed::Box<dyn Service>) -> Result<()> {
        if self.state == ServiceState::Running
            || self.services.iter().any(|registered| registered.name() == service.name())
        {
            return Err(Error::new(ErrorCode::InvalidArgument));
        }
        self.services.push(service);
        Ok(())
    }

    /// Initializes all registered services.
    pub fn start(&mut self) -> Result<()> {
        if self.state == ServiceState::Running {
            return Err(Error::new(ErrorCode::InvalidArgument));
        }
        for service in &mut self.services {
            service.init()?;
        }
        self.state = ServiceState::Running;
        Ok(())
    }

    /// Stops all services and returns the registry to the stopped state.
    pub fn stop(&mut self) {
        if self.state == ServiceState::Running {
            for service in &mut self.services {
                service.stop();
            }
            self.state = ServiceState::Stopped;
        }
    }

    /// Returns the current lifecycle state.
    pub fn state(&self) -> ServiceState {
        self.state
    }

    /// Dispatches a message to a registered service by name.
    pub fn dispatch(&mut self, name: &str, request: &Message) -> Result<Option<Message>> {
        if self.state != ServiceState::Running {
            return Err(Error::new(ErrorCode::Transport));
        }
        self.services
            .iter_mut()
            .find(|service| service.name() == name)
            .ok_or(Error::new(ErrorCode::NotFound))?
            .handle(request)
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::capability::Capability;
    use crate::ipc::PAYLOAD_SIZE;

    fn request() -> Message {
        Message::new(1, 2, message_type::REQUEST, Capability { id: 1 }, [7; PAYLOAD_SIZE])
    }

    #[test]
    fn registry_manages_service_lifecycle() {
        let mut registry = ServiceRegistry::new();
        registry
            .register(std::boxed::Box::new(EchoService::new()))
            .unwrap();
        assert_eq!(registry.state(), ServiceState::Stopped);
        assert!(registry.dispatch("echo", &request()).is_err());
        registry.start().unwrap();
        let response = registry.dispatch("echo", &request()).unwrap().unwrap();
        assert_eq!(response.sender, 2);
        assert_eq!(response.receiver, 1);
        assert_eq!(response.message_type, message_type::RESPONSE);
        registry.stop();
        assert_eq!(registry.state(), ServiceState::Stopped);
    }

    #[test]
    fn registry_rejects_duplicate_names() {
        let mut registry = ServiceRegistry::new();
        registry.register(std::boxed::Box::new(EchoService::new())).unwrap();
        assert_eq!(
            registry.register(std::boxed::Box::new(EchoService::new())),
            Err(Error::new(ErrorCode::InvalidArgument))
        );
    }
}
