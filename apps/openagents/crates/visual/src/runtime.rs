//! The browser VM returns execution failures through `Result`, including on
//! panic-abort Wasm. Keep the small API differences from native mlua here.

#[cfg(any(target_family = "wasm", feature = "portable-runtime"))]
pub use luau::{
    Error, FromLua, Function, Lua, RegistryKey, Result, Table, Userdata as UserData,
    UserdataFields as UserDataFields, UserdataMethods as UserDataMethods, Value, Variadic,
};

#[cfg(not(any(target_family = "wasm", feature = "portable-runtime")))]
pub use mlua::{
    Error, Lua, RegistryKey, Result, UserData, UserDataFields, UserDataMethods, Variadic,
};

#[cfg(not(any(target_family = "wasm", feature = "portable-runtime")))]
pub type Function<'lua> = mlua::Function;
#[cfg(not(any(target_family = "wasm", feature = "portable-runtime")))]
pub type Table<'lua> = mlua::Table;
#[cfg(not(any(target_family = "wasm", feature = "portable-runtime")))]
pub type Value<'lua> = mlua::Value;
#[cfg(not(any(target_family = "wasm", feature = "portable-runtime")))]
pub trait FromLua<'lua>: mlua::FromLua {}
#[cfg(not(any(target_family = "wasm", feature = "portable-runtime")))]
impl<T: mlua::FromLua> FromLua<'_> for T {}

pub fn new() -> Result<Lua> {
    #[cfg(any(target_family = "wasm", feature = "portable-runtime"))]
    {
        use luau::StdLib as L;
        Lua::new_with_libs(L::BASE | L::TABLE | L::STRING | L::MATH | L::UTF8 | L::BIT32)
    }
    #[cfg(not(any(target_family = "wasm", feature = "portable-runtime")))]
    {
        use mlua::StdLib as L;
        Lua::new_with(
            L::TABLE | L::STRING | L::MATH | L::UTF8 | L::BIT,
            mlua::LuaOptions::default(),
        )
    }
}

pub fn memory_limit(lua: &Lua, bytes: usize) -> Result<()> {
    #[cfg(any(target_family = "wasm", feature = "portable-runtime"))]
    lua.set_memory_limit(bytes);
    #[cfg(not(any(target_family = "wasm", feature = "portable-runtime")))]
    lua.set_memory_limit(bytes)?;
    Ok(())
}

pub fn globals(lua: &Lua) -> Result<Table<'_>> {
    #[cfg(any(target_family = "wasm", feature = "portable-runtime"))]
    {
        lua.globals()
    }
    #[cfg(not(any(target_family = "wasm", feature = "portable-runtime")))]
    {
        Ok(lua.globals())
    }
}

pub fn interrupt(lua: &mut Lua, check: impl Fn() -> Result<()> + 'static) {
    #[cfg(any(target_family = "wasm", feature = "portable-runtime"))]
    {
        let execution = std::rc::Rc::new(check);
        let pattern = execution.clone();
        lua.set_interrupt_handler(
            luau::InterruptHooks::new()
                .on_execution(move |_| {
                    execution()?;
                    Ok(luau::InterruptAction::Continue)
                })
                .on_pattern(move |_| pattern()),
        );
    }
    #[cfg(not(any(target_family = "wasm", feature = "portable-runtime")))]
    lua.set_interrupt(move |_| {
        check()?;
        Ok(mlua::VmState::Continue)
    });
}

pub fn clone_table<'lua>(table: &Table<'lua>) -> Result<Table<'lua>> {
    #[cfg(any(target_family = "wasm", feature = "portable-runtime"))]
    {
        table.try_clone()
    }
    #[cfg(not(any(target_family = "wasm", feature = "portable-runtime")))]
    {
        Ok(table.clone())
    }
}

// The two runtimes differ only in how typed callback arguments and results
// cross the VM stack. The drawing and scene implementations stay shared.
macro_rules! method {
    ($methods:expr, $name:expr, |$lua:tt, $this:ident, $arg:tt: $ty:ty| $body:block $(,)?) => {
        #[cfg(not(any(target_family = "wasm", feature = "portable-runtime")))]
        $methods.add_method_mut($name, |$lua, $this, $arg: $ty| $body);
        #[cfg(any(target_family = "wasm", feature = "portable-runtime"))]
        $methods.add_method_mut($name, |$lua, $this, mut args| {
            let $arg: $ty = args.take_remaining()?;
            let result: crate::runtime::Result<_> = (|| $body)();
            args.finish(result?)
        });
    };
}

macro_rules! field {
    ($fields:expr, $name:expr, |$lua:tt, $this:ident| $body:expr) => {
        #[cfg(not(any(target_family = "wasm", feature = "portable-runtime")))]
        $fields.add_field_method_get($name, |$lua, $this| $body);
        #[cfg(any(target_family = "wasm", feature = "portable-runtime"))]
        $fields.add_field_method_get($name, |$lua, $this, args| {
            let result: crate::runtime::Result<_> = (|| $body)();
            args.finish(result?)
        });
    };
}

macro_rules! function {
    ($lua:expr, $capture:tt |$context:tt, $arg:tt: $ty:ty| $body:block) => {{
        #[cfg(not(any(target_family = "wasm", feature = "portable-runtime")))]
        let f = $lua.create_function($capture |$context, $arg: $ty| $body);
        #[cfg(any(target_family = "wasm", feature = "portable-runtime"))]
        let f = $lua.create_function($capture |$context, mut args| {
            let $arg: $ty = args.take_remaining()?;
            let result: crate::runtime::Result<_> = (|| $body)();
            args.finish(result?)
        });
        f
    }};
}

pub(crate) use {field, function, method};

#[cfg(any(target_family = "wasm", feature = "portable-runtime"))]
pub fn string_text(s: &luau::LuaString<'_>) -> String {
    s.to_str_lossy().into_owned()
}
#[cfg(not(any(target_family = "wasm", feature = "portable-runtime")))]
pub fn string_text(s: &mlua::LuaString) -> String {
    s.to_string_lossy()
}
