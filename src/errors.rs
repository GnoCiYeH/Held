use error_chain::error_chain;

impl Default for Error {
    fn default() -> Self {
        Self(ErrorKind::Unreachable, Default::default())
    }
}

error_chain! {
    errors {
        EmptyWorkspace {
            description("the workspace is empty")
            display("the workspace is empty")
        }
        MissingBuffer {
            description("the buffer not found")
            display("the buffer not found")
        }
        MissingPath {
            description("buffer doesn't have a path")
            display("buffer doesn't have a path")
        }
        MissingScope {
            description("couldn't find any scopes at the cursor position")
            display("couldn't find any scopes at the cursor position")
        }
        MissingSyntax {
            description("no syntax definition for the current buffer")
            display("no syntax definition for the current buffer")
        }
        Unreachable {
            description("Unreachable error")
            display("Unreachable error")
        }
    }

    foreign_links {
        Io(std::io::Error) #[cfg(unix)];
        ParsingError(syntect::parsing::ParsingError);
        ScopeError(syntect::parsing::ScopeError);
        SyntaxLoadingError(syntect::LoadingError);
        DlopenError(dlopen2::Error);
    }
}
