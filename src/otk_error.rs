quick_error! {
    #[derive(Debug)]
    pub enum OTKError {
        ParseError(err: String) {
            display("Parsing Error: {}", err)
        }
        UnimplementedError(err: String) {
            display("Unimplemented: {}", err)
        }
        InvalidArgumentError(err: String) {
            display("Invalid argument: {}", err)
        }
    }
}
