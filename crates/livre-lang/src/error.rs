use std::fmt;

/// A parse or runtime error, with the 1-based line (within the code block) it occurred on.
#[derive(Debug, Clone, PartialEq)]
pub struct LangError {
    pub message: String,
    pub line: usize,
    /// MATLAB's error identifier (`'MATLAB:validators:mustBePositive'`); empty when none.
    pub identifier: String,
    /// Where the error happened, innermost first (`MException.stack`).
    pub stack: Vec<ErrFrame>,
    /// The stack has been recorded (it is taken once, where the error first leaves a statement).
    pub captured: bool,
    /// Raised by `error` or `throw` (the report starts "Error using <function>").
    pub raised: bool,
    /// Innermost frames to leave out of the stack (`throwAsCaller` drops its caller's).
    pub drop_frames: usize,
}

/// One frame of an error's stack: the function (or script) and the line it was running.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ErrFrame {
    pub name: String,
    pub file: String,
    pub line: usize,
}

impl LangError {
    pub fn new(message: impl Into<String>, line: usize) -> Self {
        Self { message: message.into(), line, identifier: String::new(), stack: Vec::new(), captured: false, raised: false, drop_frames: 0 }
    }

    /// Marks the error as raised by `error` or `throw` in user code.
    pub fn raised(mut self) -> Self {
        self.raised = true;
        self
    }

    pub fn with_id(mut self, identifier: impl Into<String>) -> Self {
        self.identifier = identifier.into();
        self
    }
}

impl fmt::Display for LangError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for LangError {}

/// Shorthand for building a runtime error before the line is known; the interpreter fills in
/// the line of the statement being executed.
pub fn rt<T>(message: impl Into<String>) -> Result<T, LangError> {
    Err(LangError::new(message, 0))
}

/// A runtime error with an identifier.
pub fn rt_id<T>(identifier: &str, message: impl Into<String>) -> Result<T, LangError> {
    Err(LangError::new(message, 0).with_id(identifier))
}
