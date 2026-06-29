use mlua::{Lua, Table};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::path::Path;

pub struct WidgetInfo {
    pub name: String,
    pub text: String,
    pub color: String,
}

pub struct LuaEngine {
    lua: Lua,
    cpu_usage: Arc<AtomicU32>,
}

impl LuaEngine {
    /// Creates a new LuaEngine and initializes the shared CPU usage atomic variable.
    pub fn new(cpu_usage: Arc<AtomicU32>) -> mlua::Result<Self> {
        let lua = Lua::new();
        
        let engine = Self { lua, cpu_usage };
        engine.setup_sysmon_module()?;
        
        Ok(engine)
    }

    /// Sets up the preloaded `sysmon` module in the Lua package.preload table.
    fn setup_sysmon_module(&self) -> mlua::Result<()> {
        let package: Table = self.lua.globals().get("package")?;
        let preload: Table = package.get("preload")?;
        
        let cpu_usage_shared = self.cpu_usage.clone();
        
        let sysmon_preload = self.lua.create_function(move |lua, _: ()| {
            let sysmon = lua.create_table()?;
            
            // Table to hold all registered widgets config
            sysmon.set("_widgets", lua.create_table()?)?;
            
            // sysmon.get_cpu_usage()
            let cpu_usage_fn = cpu_usage_shared.clone();
            let get_cpu_usage = lua.create_function(move |_, _: ()| {
                let val = cpu_fn_to_f32(&cpu_usage_fn);
                Ok(val)
            })?;
            sysmon.set("get_cpu_usage", get_cpu_usage)?;
            
            // sysmon.register_widget(name, config)
            let register_widget = lua.create_function(|lua, (name, config): (String, Table)| {
                let package: Table = lua.globals().get("package")?;
                let loaded: Table = package.get("loaded")?;
                let sysmon: Table = loaded.get("sysmon")?;
                let widgets: Table = sysmon.get("_widgets")?;
                widgets.set(name, config)?;
                Ok(())
            })?;
            sysmon.set("register_widget", register_widget)?;
            
            Ok(sysmon)
        })?;
        
        preload.set("sysmon", sysmon_preload)?;
        Ok(())
    }

    /// Loads and runs a Lua file (e.g. config.lua)
    pub fn load_config<P: AsRef<Path>>(&self, path: P) -> mlua::Result<()> {
        let content = std::fs::read_to_string(path)?;
        self.lua.load(&content).exec()?;
        Ok(())
    }

    /// Iterates through the registered Lua widgets and executes their render functions.
    pub fn get_widgets(&self) -> mlua::Result<Vec<WidgetInfo>> {
        let package: Table = self.lua.globals().get("package")?;
        let loaded: Table = package.get("loaded")?;
        
        // If sysmon is not in package.loaded, then require("sysmon") hasn't run.
        let sysmon: Option<Table> = loaded.get("sysmon").ok();
        let sysmon = match sysmon {
            Some(t) => t,
            None => return Ok(Vec::new()),
        };
        
        let widgets: Table = sysmon.get("_widgets")?;
        let mut result = Vec::new();
        
        for pair in widgets.pairs::<String, Table>() {
            let (name, config) = pair?;
            let render_fn: Option<mlua::Function> = config.get("render").ok();
            
            if let Some(render_fn) = render_fn {
                // Call Lua render function, capturing any errors and returning them as red text
                match render_fn.call::<_, (String, String)>(()) {
                    Ok((text, color)) => {
                        result.push(WidgetInfo { name, text, color });
                    }
                    Err(e) => {
                        result.push(WidgetInfo {
                            name,
                            text: format!("Lua Error: {}", e),
                            color: "red".to_string(),
                        });
                    }
                }
            } else {
                result.push(WidgetInfo {
                    name: name.clone(),
                    text: format!("Widget '{}' has no render function", name),
                    color: "yellow".to_string(),
                });
            }
        }
        
        Ok(result)
    }
}

// Helper to convert atomic integer back to float
fn cpu_fn_to_f32(atomic: &AtomicU32) -> f32 {
    atomic.load(Ordering::Relaxed) as f32 / 10.0
}
