use mlua::{Lua, Table};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::path::Path;

pub struct WidgetInfo {
    pub name: String,
    pub text: String,
    pub color: String,
}

pub struct LuaEngine {
    lua: Lua,
    cpu_usage: Arc<AtomicU32>,
    total_mem: Arc<AtomicU64>,
    used_mem: Arc<AtomicU64>,
    mem_percent: Arc<AtomicU32>,
    nvml: Arc<Option<nvml_wrapper::Nvml>>,
}

impl LuaEngine {
    /// Creates a new LuaEngine and initializes it with the shared metrics.
    pub fn new(
        cpu_usage: Arc<AtomicU32>,
        total_mem: Arc<AtomicU64>,
        used_mem: Arc<AtomicU64>,
        mem_percent: Arc<AtomicU32>,
        nvml: Option<nvml_wrapper::Nvml>,
    ) -> mlua::Result<Self> {
        let lua = Lua::new();
        let nvml = Arc::new(nvml);
        
        let engine = Self {
            lua,
            cpu_usage,
            total_mem,
            used_mem,
            mem_percent,
            nvml,
        };
        engine.setup_sysmon_module()?;
        
        Ok(engine)
    }

    /// Sets up the preloaded `sysmon` module in the Lua package.preload table.
    fn setup_sysmon_module(&self) -> mlua::Result<()> {
        let package: Table = self.lua.globals().get("package")?;
        let preload: Table = package.get("preload")?;
        
        let cpu_usage_shared = self.cpu_usage.clone();
        let total_mem_shared = self.total_mem.clone();
        let used_mem_shared = self.used_mem.clone();
        let mem_percent_shared = self.mem_percent.clone();
        let nvml_shared = self.nvml.clone();
        
        let sysmon_preload = self.lua.create_function(move |lua, _: ()| {
            let sysmon = lua.create_table()?;
            
            // Table to hold all registered widgets config
            sysmon.set("_widgets", lua.create_table()?)?;
            
            // --- CPU APIs ---
            
            // sysmon.get_cpu_usage()
            let cpu_usage_fn = cpu_usage_shared.clone();
            let get_cpu_usage = lua.create_function(move |_, _: ()| {
                let val = cpu_usage_fn.load(Ordering::Relaxed) as f32 / 10.0;
                Ok(val)
            })?;
            sysmon.set("get_cpu_usage", get_cpu_usage)?;
            
            // --- RAM APIs ---
            
            // sysmon.get_total_memory()
            let total_mem_fn = total_mem_shared.clone();
            let get_total_memory = lua.create_function(move |_, _: ()| {
                Ok(total_mem_fn.load(Ordering::Relaxed))
            })?;
            sysmon.set("get_total_memory", get_total_memory)?;
            
            // sysmon.get_used_memory()
            let used_mem_fn = used_mem_shared.clone();
            let get_used_memory = lua.create_function(move |_, _: ()| {
                Ok(used_mem_fn.load(Ordering::Relaxed))
            })?;
            sysmon.set("get_used_memory", get_used_memory)?;
            
            // sysmon.get_memory_percent()
            let mem_percent_fn = mem_percent_shared.clone();
            let get_memory_percent = lua.create_function(move |_, _: ()| {
                let val = mem_percent_fn.load(Ordering::Relaxed) as f32 / 10.0;
                Ok(val)
            })?;
            sysmon.set("get_memory_percent", get_memory_percent)?;
            
            // --- GPU APIs (NVIDIA NVML) ---
            
            // sysmon.get_gpu_usage()
            let nvml_gpu = nvml_shared.clone();
            let get_gpu_usage = lua.create_function(move |_, _: ()| {
                if let Some(ref n) = *nvml_gpu {
                    if let Ok(device) = n.device_by_index(0) {
                        if let Ok(rates) = device.utilization_rates() {
                            return Ok(rates.gpu as f32);
                        }
                    }
                }
                Ok(-1.0)
            })?;
            sysmon.set("get_gpu_usage", get_gpu_usage)?;
            
            // sysmon.get_gpu_memory_used()
            let nvml_gmem_used = nvml_shared.clone();
            let get_gpu_memory_used = lua.create_function(move |_, _: ()| {
                if let Some(ref n) = *nvml_gmem_used {
                    if let Ok(device) = n.device_by_index(0) {
                        if let Ok(mem) = device.memory_info() {
                            return Ok(mem.used as i64);
                        }
                    }
                }
                Ok(-1)
            })?;
            sysmon.set("get_gpu_memory_used", get_gpu_memory_used)?;
            
            // sysmon.get_gpu_memory_total()
            let nvml_gmem_total = nvml_shared.clone();
            let get_gpu_memory_total = lua.create_function(move |_, _: ()| {
                if let Some(ref n) = *nvml_gmem_total {
                    if let Ok(device) = n.device_by_index(0) {
                        if let Ok(mem) = device.memory_info() {
                            return Ok(mem.total as i64);
                        }
                    }
                }
                Ok(-1)
            })?;
            sysmon.set("get_gpu_memory_total", get_gpu_memory_total)?;
            
            // sysmon.get_gpu_name()
            let nvml_name = nvml_shared.clone();
            let get_gpu_name = lua.create_function(move |_, _: ()| {
                if let Some(ref n) = *nvml_name {
                    if let Ok(device) = n.device_by_index(0) {
                        if let Ok(name) = device.name() {
                            return Ok(name);
                        }
                    }
                }
                Ok("N/A".to_string())
            })?;
            sysmon.set("get_gpu_name", get_gpu_name)?;
            
            // --- General Widget Registry ---
            
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
        
        // Sort widgets alphabetically by name to guarantee stable rendering order
        result.sort_by(|a, b| a.name.cmp(&b.name));
        
        Ok(result)
    }
}
