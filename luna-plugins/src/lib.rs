use mlua::{Function, Lua, RegistryKey, Value};
use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::rc::Rc;

pub struct RegisteredCommand {
    pub phrases: Vec<String>,
    pub handler: RegistryKey,
    pub plugin: String,
}

pub struct PluginManager {
    lua: Lua,
    pending: Rc<RefCell<Vec<RegisteredCommand>>>,
    pub commands: Vec<RegisteredCommand>,
}

// Pomocnik: zamiana dowolnego Display na mlua::Error
fn err<E: std::fmt::Display>(e: E) -> mlua::Error {
    mlua::Error::RuntimeError(e.to_string())
}

impl PluginManager {
    pub fn new() -> mlua::Result<Self> {
        let lua = Lua::new();
        let pending: Rc<RefCell<Vec<RegisteredCommand>>> = Rc::new(RefCell::new(Vec::new()));

        install_luna_api(&lua, &pending)?;
        apply_sandbox(&lua)?;

        Ok(Self { lua, pending, commands: Vec::new() })
    }

    pub fn load_dir(&mut self, dir: impl AsRef<Path>) -> mlua::Result<usize> {
        let dir = dir.as_ref();
        if !dir.exists() {
            fs::create_dir_all(dir).map_err(err)?;
        }
        let mut count = 0;
        for entry in fs::read_dir(dir).map_err(err)? {
            let path = entry.map_err(err)?.path();
            if path.extension().and_then(|s| s.to_str()) == Some("lua") {
                self.load_file(&path)?;
                count += 1;
            }
        }
        let mut p = self.pending.borrow_mut();
        self.commands.append(&mut *p);
        Ok(count)
    }

    fn load_file(&self, path: &Path) -> mlua::Result<()> {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let src = fs::read_to_string(path).map_err(err)?;

        self.lua.globals().set("__current_plugin", name.as_str())?;
        self.lua
        .load(&src)
        .set_name(&name)
        .exec()?;
        self.lua.globals().set("__current_plugin", Value::Nil)?;
        Ok(())
    }

    pub fn process(&self, text: &str) -> mlua::Result<Option<String>> {
        let lower = text.to_lowercase();
        for cmd in &self.commands {
            let matched = cmd
            .phrases
            .iter()
            .any(|p| !p.is_empty() && lower.contains(&p.to_lowercase()));
            if !matched {
                continue;
            }

            let func: Function = self.lua.registry_value(&cmd.handler)?;
            // Tylko JEDEN generyk: typ zwracany. Argumenty idą jako impl IntoLuaMulti.
            match func.call::<Value>(text.to_string()) {
                Ok(Value::Boolean(true)) => {
                    return Ok(Some(format!("(handled by '{}')", cmd.plugin)));
                }
                Ok(Value::String(s)) => {
                    let s = s.to_str()?.to_owned();
                    return Ok(Some(s));
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("[plugin '{}'] error: {}", cmd.plugin, e);
                }
            }
        }
        Ok(None)
    }
}

fn install_luna_api(lua: &Lua, pending: &Rc<RefCell<Vec<RegisteredCommand>>>) -> mlua::Result<()> {
    let luna = lua.create_table()?;

    luna.set(
        "log",
        lua.create_function(|_, msg: String| {
            println!("[lua] {}", msg);
            Ok(())
        })?,
    )?;

    luna.set(
        "speak",
        lua.create_function(|_, msg: String| {
            println!("[luna/speak] {}", msg);
            Ok(())
        })?,
    )?;

    let p = pending.clone();
    luna.set(
        "command",
        lua.create_function(move |lua, (phrases, handler): (Value, Function)| {
            let phrase_list = match phrases {
                Value::String(s) => vec![s.to_str()?.to_owned()],
                            Value::Table(t) => t
                            .sequence_values::<String>()
                            .collect::<mlua::Result<Vec<_>>>()?,
                            _ => {
                                return Err(mlua::Error::runtime(
                                    "phrases must be a string or a table of strings",
                                ))
                            }
            };
            let plugin = lua
            .globals()
            .get::<String>("__current_plugin")
            .unwrap_or_else(|_| "unknown".into());
            let key = lua.create_registry_value(handler)?;
            p.borrow_mut().push(RegisteredCommand {
                phrases: phrase_list,
                handler: key,
                plugin,
            });
            Ok(())
        })?,
    )?;

    lua.globals().set("luna", luna)?;
    Ok(())
}

fn apply_sandbox(lua: &Lua) -> mlua::Result<()> {
    let g = lua.globals();
    for dangerous in ["os", "io", "package", "debug", "require", "load", "loadstring", "dofile", "loadfile"] {
        g.set(dangerous, Value::Nil)?;
    }
    g.set(
        "print",
        lua.create_function(|_, msg: String| {
            println!("[lua/print] {}", msg);
            Ok(())
        })?,
    )?;
    Ok(())
}
