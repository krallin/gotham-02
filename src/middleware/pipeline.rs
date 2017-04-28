//! Defines types for a middleware pipeline

use std::io;
use middleware::{Middleware, NewMiddleware};
use handler::{NewHandler, Handler, HandlerFuture};
use state::State;
use hyper::server::Request;
use futures::{future, Future};

// TODO: Refactor this example when the `Router` API properly integrates with pipelines.
/// When using middleware, one or more [`Middleware`][Middleware] are combined to form a
/// `Pipeline`. `Middleware` are invoked strictly in the order they're added to the `Pipeline`.
///
/// At request dispatch time, the `Middleware` are created from the
/// [`NewMiddleware`][NewMiddleware] values given to the `PipelineBuilder`, and combined with a
/// [`Handler`][Handler] created from the [`NewHandler`][NewHandler] provided to `Pipeline::call`.
/// These `Middleware` and `Handler` values are used for a single request.
///
/// [Middleware]: ../trait.Middleware.html
/// [NewMiddleware]: ../trait.NewMiddleware.html
/// [Handler]: ../../handler/trait.Handler.html
/// [NewHandler]: ../../handler/trait.NewHandler.html
///
/// # Examples
///
/// ```rust
/// # extern crate gotham;
/// # extern crate hyper;
/// # extern crate futures;
/// #
/// # use std::io;
/// # use gotham::state::{State, StateData};
/// # use gotham::handler::{Handler, HandlerFuture, HandlerService, NewHandlerService};
/// # use gotham::middleware::{Middleware, NewMiddleware};
/// # use gotham::middleware::pipeline::{new_pipeline, Pipeline, PipelineBuilder};
/// # use gotham::router::Router;
/// # use gotham::test::TestServer;
/// # use hyper::server::{Request, Response};
/// # use hyper::StatusCode;
/// # use hyper::Method::*;
/// #
/// struct MiddlewareData {
///     vec: Vec<i32>
/// }
///
/// impl StateData for MiddlewareData {}
///
/// # #[derive(Clone)]
/// struct MiddlewareOne;
/// # #[derive(Clone)]
/// struct MiddlewareTwo;
/// # #[derive(Clone)]
/// struct MiddlewareThree;
///
/// impl Middleware for MiddlewareOne {
///     // Implementation elided.
///     // Appends `1` to `MiddlewareData.vec`
/// #     fn call<Chain>(self, mut state: State, req: Request, chain: Chain) -> Box<HandlerFuture>
/// #         where Chain: FnOnce(State, Request) -> Box<HandlerFuture> + Send + 'static
/// #     {
/// #         state.put(MiddlewareData { vec: vec![1] });
/// #         chain(state, req)
/// #     }
/// }
/// #
/// # impl NewMiddleware for MiddlewareOne {
/// #     type Instance = MiddlewareOne;
/// #     fn new_middleware(&self) -> io::Result<MiddlewareOne> {
/// #         Ok(self.clone())
/// #     }
/// # }
///
/// impl Middleware for MiddlewareTwo {
///     // Implementation elided.
///     // Appends `2` to `MiddlewareData.vec`
/// #     fn call<Chain>(self, mut state: State, req: Request, chain: Chain) -> Box<HandlerFuture>
/// #         where Chain: FnOnce(State, Request) -> Box<HandlerFuture> + Send + 'static
/// #     {
/// #         state.borrow_mut::<MiddlewareData>().unwrap().vec.push(2);
/// #         chain(state, req)
/// #     }
/// }
/// #
/// # impl NewMiddleware for MiddlewareTwo {
/// #     type Instance = MiddlewareTwo;
/// #     fn new_middleware(&self) -> io::Result<MiddlewareTwo> {
/// #         Ok(self.clone())
/// #     }
/// # }
///
/// impl Middleware for MiddlewareThree {
///     // Implementation elided.
///     // Appends `3` to `MiddlewareData.vec`
/// #     fn call<Chain>(self, mut state: State, req: Request, chain: Chain) -> Box<HandlerFuture>
/// #         where Chain: FnOnce(State, Request) -> Box<HandlerFuture> + Send + 'static
/// #     {
/// #         state.borrow_mut::<MiddlewareData>().unwrap().vec.push(3);
/// #         chain(state, req)
/// #     }
/// }
/// #
/// # impl NewMiddleware for MiddlewareThree {
/// #     type Instance = MiddlewareThree;
/// #     fn new_middleware(&self) -> io::Result<MiddlewareThree> {
/// #         Ok(self.clone())
/// #     }
/// # }
///
/// fn handler(mut state: State, req: Request) -> (State, Response) {
///     // Dump the contents of the `Vec<i32>` into the response body.
///     let body = {
///         let data = state.borrow::<MiddlewareData>().unwrap();
///         format!("{:?}", data.vec)
///     };
///
///     (state, Response::new().with_status(StatusCode::Ok).with_body(body))
/// }
///
/// fn main() {
///     let new_service = NewHandlerService::new(|| {
///         // Define a `Router`
///         let router = Router::build(|routes| {
///             routes.direct(Get, "/").to(handler);
///         });
///
///         // Build the `Pipeline`
///         let pipeline = new_pipeline()
///             .add(MiddlewareOne)
///             .add(MiddlewareTwo)
///             .add(MiddlewareThree)
///             .build();
///
///         // Return the `Pipeline` as a `Handler`
///         Ok(move |state, req| pipeline.call(&router, state, req))
///     });
///
///     let mut test_server = TestServer::new(new_service).unwrap();
///     let client = test_server.client("127.0.0.1:10000".parse().unwrap()).unwrap();
///     let uri = "http://example.com/".parse().unwrap();
///     let response = test_server.run_request(client.get(uri)).unwrap();
///     assert_eq!(response.status(), StatusCode::Ok);
///     assert_eq!(test_server.read_body(response).unwrap(), "[1, 2, 3]".as_bytes());
/// }
/// ```
pub struct Pipeline<T>
    where T: NewPipelineInstance
{
    builder: PipelineBuilder<T>,
}

impl<T> Pipeline<T>
    where T: NewPipelineInstance
{
    /// Invokes the `Pipeline`, which will execute all middleware in the order provided via
    /// `PipelineBuilder::add` and then process requests via the `Handler` instance created by the
    /// `NewHandler`.
    pub fn call<H>(&self, new_handler: &H, state: State, req: Request) -> Box<HandlerFuture>
        where H: NewHandler,
              H::Instance: 'static
    {
        // Creates the per-request `Handler` and `Middleware` instances, and then calls to them.
        match new_handler.new_handler() {
            Ok(handler) => {
                match self.builder.t.new_pipeline_instance() {
                    Ok(p) => p.call(state, req, handler), // See: `PipelineInstance::call`
                    Err(e) => future::err((state, e.into())).boxed(),
                }
            }
            Err(e) => future::err((state, e.into())).boxed(),
        }
    }
}

/// Begins defining a new pipeline.
///
/// See [`PipelineBuilder`][PipelineBuilder] for information on using `new_pipeline()`
///
/// [PipelineBuilder]: struct.PipelineBuilder.html
pub fn new_pipeline() -> PipelineBuilder<()> {
    // See: `impl NewPipelineInstance for ()`
    PipelineBuilder { t: () }
}

/// Allows a pipeline to be defined by adding `NewMiddleware` values, and building a `Pipeline`.
///
/// # Examples
///
/// ```rust
/// # extern crate gotham;
/// # extern crate hyper;
/// # extern crate futures;
/// #
/// # use std::io;
/// # use gotham::state::State;
/// # use gotham::handler::{Handler, HandlerFuture};
/// # use gotham::middleware::{Middleware, NewMiddleware};
/// # use gotham::middleware::pipeline::{new_pipeline, Pipeline, PipelineBuilder};
/// # use hyper::server::{Request, Response};
/// # use hyper::StatusCode;
/// #
/// # #[derive(Clone)]
/// # struct MiddlewareOne;
/// # #[derive(Clone)]
/// # struct MiddlewareTwo;
/// # #[derive(Clone)]
/// # struct MiddlewareThree;
/// #
/// # impl Middleware for MiddlewareOne {
/// #   fn call<Chain>(self, state: State, req: Request, chain: Chain) -> Box<HandlerFuture>
/// #       where Chain: FnOnce(State, Request) -> Box<HandlerFuture> + Send + 'static
/// #   {
/// #       chain(state, req)
/// #   }
/// # }
/// #
/// # impl NewMiddleware for MiddlewareOne {
/// #   type Instance = MiddlewareOne;
/// #   fn new_middleware(&self) -> io::Result<MiddlewareOne> {
/// #       Ok(self.clone())
/// #   }
/// # }
/// #
/// # impl Middleware for MiddlewareTwo {
/// #   fn call<Chain>(self, state: State, req: Request, chain: Chain) -> Box<HandlerFuture>
/// #       where Chain: FnOnce(State, Request) -> Box<HandlerFuture> + Send + 'static
/// #   {
/// #       chain(state, req)
/// #   }
/// # }
/// #
/// # impl NewMiddleware for MiddlewareTwo {
/// #   type Instance = MiddlewareTwo;
/// #   fn new_middleware(&self) -> io::Result<MiddlewareTwo> {
/// #       Ok(self.clone())
/// #   }
/// # }
/// #
/// # impl Middleware for MiddlewareThree {
/// #   fn call<Chain>(self, state: State, req: Request, chain: Chain) -> Box<HandlerFuture>
/// #       where Chain: FnOnce(State, Request) -> Box<HandlerFuture> + Send + 'static
/// #   {
/// #       chain(state, req)
/// #   }
/// # }
/// #
/// # impl NewMiddleware for MiddlewareThree {
/// #   type Instance = MiddlewareThree;
/// #   fn new_middleware(&self) -> io::Result<MiddlewareThree> {
/// #       Ok(self.clone())
/// #   }
/// # }
/// #
/// # fn handler(state: State, _: Request) -> (State, Response) {
/// #   (state, Response::new().with_status(StatusCode::Accepted))
/// # }
/// #
/// # fn main() {
/// let pipeline: Pipeline<_> = new_pipeline()
///     .add(MiddlewareOne)
///     .add(MiddlewareTwo)
///     .add(MiddlewareThree)
///     .build();
/// # }
/// ```
///
/// The pipeline defined here is invoked in this order:
///
/// `(&mut state, request)` &rarr; `MiddlewareOne` &rarr; `MiddlewareTwo` &rarr; `MiddlewareThree`
/// &rarr; `handler` (provided later)
pub struct PipelineBuilder<T>
    where T: NewPipelineInstance
{
    t: T,
}

impl<T> PipelineBuilder<T>
    where T: NewPipelineInstance
{
    /// Builds a `Pipeline`, which contains all middleware in the order provided via `add` and is
    /// ready to process requests via a `NewHandler` provided to [`Pipeline::call`][Pipeline::call]
    ///
    /// [Pipeline::call]: struct.Pipeline.html#method.call
    pub fn build(self) -> Pipeline<T>
        where T: NewPipelineInstance
    {
        Pipeline { builder: self }
    }

    /// Adds a `NewMiddleware` which will create a `Middleware` during request dispatch.
    pub fn add<M>(self, m: M) -> PipelineBuilder<(M, T)>
        where M: NewMiddleware,
              M::Instance: Send + 'static,
              Self: Sized
    {
        // "cons" the most recently added `NewMiddleware` onto the front of the list. This is
        // essentially building an HList-style tuple in reverse order. So for a call like:
        //
        //     new_pipeline().add(MiddlewareOne).add(MiddlewareTwo).add(MiddlewareThree)
        //
        // The resulting `PipelineBuilder` will be:
        //
        //     PipelineBuilder { t: (MiddlewareThree, (MiddlewareTwo, (MiddlewareOne, ()))) }
        //
        // An empty `PipelineBuilder` is represented as:
        //
        //     PipelineBuilder { t: () }
        PipelineBuilder { t: (m, self.t) }
    }
}

/// A recursive type representing a pipeline, which is used to spawn a `PipelineInstance`.
///
/// This type should never be implemented outside of Gotham, does not form part of the public API,
/// and is subject to change without notice.
#[doc(hidden)]
pub unsafe trait NewPipelineInstance: Sized {
    type Instance: PipelineInstance;

    /// Create and return a new `PipelineInstance` value.
    fn new_pipeline_instance(&self) -> io::Result<Self::Instance>;
}

unsafe impl<T, U> NewPipelineInstance for (T, U)
    where T: NewMiddleware,
          T::Instance: Send + 'static,
          U: NewPipelineInstance
{
    type Instance = (T::Instance, U::Instance);

    fn new_pipeline_instance(&self) -> io::Result<Self::Instance> {
        // This works as a recursive `map` over the "list" of `NewMiddleware`, and is used in
        // creating the `Middleware` instances for serving a single request.
        //
        // The reversed order is preserved in the return value.
        let (ref nm, ref tail) = *self;
        Ok((nm.new_middleware()?, tail.new_pipeline_instance()?))
    }
}

unsafe impl NewPipelineInstance for () {
    type Instance = ();

    fn new_pipeline_instance(&self) -> io::Result<Self::Instance> {
        // () marks the end of the list, so is returned as-is.
        Ok(())
    }
}

/// A recursive type representing an instance of a pipeline, which is used to process a single
/// request.
///
/// This type should never be implemented outside of Gotham, does not form part of the public API,
/// and is subject to change without notice.
#[doc(hidden)]
pub unsafe trait PipelineInstance: Sized {
    /// Dispatches a request to the given `Handler` after processing all `Middleware` in the
    /// pipeline.
    fn call<H>(self, state: State, request: Request, handler: H) -> Box<HandlerFuture>
        where H: Handler + 'static
    {
        // Entry point into the `PipelineInstance`. Begins recursively constructing a function,
        // starting with a function which invokes the `Handler`.
        self.call_recurse(state, request, move |state, req| handler.handle(state, req))
    }

    /// Recursive function for processing middleware and chaining to the given function.
    fn call_recurse<F>(self, state: State, request: Request, f: F) -> Box<HandlerFuture>
        where F: FnOnce(State, Request) -> Box<HandlerFuture> + Send + 'static;
}

unsafe impl PipelineInstance for () {
    fn call_recurse<F>(self, state: State, request: Request, f: F) -> Box<HandlerFuture>
        where F: FnOnce(State, Request) -> Box<HandlerFuture> + Send + 'static
    {
        // At the last item in the `PipelineInstance`, the function is invoked to serve the
        // request. `f` is the nested function of all `Middleware` and the `Handler`.
        //
        // In the case of 0 middleware, `f` is the function created in `PipelineInstance::call`
        // which invokes the `Handler` directly.
        f(state, request)
    }
}

unsafe impl<T, U> PipelineInstance for (T, U)
    where T: Middleware + Send + 'static,
          U: PipelineInstance
{
    fn call_recurse<F>(self, state: State, request: Request, f: F) -> Box<HandlerFuture>
        where F: FnOnce(State, Request) -> Box<HandlerFuture> + Send + 'static
    {
        let (m, p) = self;
        // Construct the function from the inside, out. Starting with a function which calls the
        // `Handler`, and then creating a new function which calls the `Middleware` with the
        // previous function as the `chain` argument, we end up with a structure somewhat like
        // this (using `m0`, `m1`, `m2` as middleware names, where `m2` is the last middleware
        // before the `Handler`):
        //
        //  move |state, req| {
        //      m0.call(state, req, move |state, req| {
        //          m1.call(state, req, move |state, req| {
        //              m2.call(state, req, move |state, req| handler.call(state, req))
        //          })
        //      })
        //  }
        //
        // The resulting function is called by `<() as PipelineInstance>::call_recurse`
        p.call_recurse(state, request, move |state, req| m.call(state, req, f))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test::TestServer;
    use handler::NewHandlerService;
    use state::StateData;
    use hyper::server::Response;
    use hyper::StatusCode;

    fn handler(state: State, _req: Request) -> (State, Response) {
        let number = state.borrow::<Number>().unwrap().value;
        (state, Response::new().with_status(StatusCode::Ok).with_body(format!("{}", number)))
    }

    #[derive(Clone)]
    struct Number {
        value: i32,
    }

    impl NewMiddleware for Number {
        type Instance = Number;

        fn new_middleware(&self) -> io::Result<Number> {
            Ok(self.clone())
        }
    }

    impl Middleware for Number {
        fn call<Chain>(self, mut state: State, req: Request, chain: Chain) -> Box<HandlerFuture>
            where Chain: FnOnce(State, Request) -> Box<HandlerFuture> + Send + 'static,
                  Self: Sized
        {
            state.put(self.clone());
            chain(state, req)
        }
    }

    impl StateData for Number {}

    struct Addition {
        value: i32,
    }

    impl NewMiddleware for Addition {
        type Instance = Addition;

        fn new_middleware(&self) -> io::Result<Addition> {
            Ok(Addition { ..*self })
        }
    }

    impl Middleware for Addition {
        fn call<Chain>(self, mut state: State, req: Request, chain: Chain) -> Box<HandlerFuture>
            where Chain: FnOnce(State, Request) -> Box<HandlerFuture> + Send + 'static,
                  Self: Sized
        {
            state.borrow_mut::<Number>().unwrap().value += self.value;
            chain(state, req)
        }
    }

    struct Multiplication {
        value: i32,
    }

    impl NewMiddleware for Multiplication {
        type Instance = Multiplication;

        fn new_middleware(&self) -> io::Result<Multiplication> {
            Ok(Multiplication { ..*self })
        }
    }

    impl Middleware for Multiplication {
        fn call<Chain>(self, mut state: State, req: Request, chain: Chain) -> Box<HandlerFuture>
            where Chain: FnOnce(State, Request) -> Box<HandlerFuture> + Send + 'static,
                  Self: Sized
        {
            state.borrow_mut::<Number>().unwrap().value *= self.value;
            chain(state, req)
        }
    }

    #[test]
    fn pipeline_ordering_test() {
        let new_service = NewHandlerService::new(|| {
            let pipeline = new_pipeline()
                .add(Number { value: 0 }) // 0
                .add(Addition { value: 1 }) // 1
                .add(Multiplication { value: 2 }) // 2
                .add(Addition { value: 1 }) // 3
                .add(Multiplication { value: 2 }) // 6
                .add(Addition { value: 2 }) // 8
                .add(Multiplication { value: 3 }) // 24
                .build();
            Ok(move |state, req| pipeline.call(&|| Ok(handler), state, req))
        });

        let uri = "http://localhost/".parse().unwrap();

        let mut test_server = TestServer::new(new_service).unwrap();
        let response = test_server.client("127.0.0.1:0".parse().unwrap()).unwrap().get(uri);
        let response = test_server.run_request(response).unwrap();

        let buf = test_server.read_body(response).unwrap();
        assert_eq!(buf.as_slice(), "24".as_bytes());
    }
}
