-- config.lua
-- This file configures the terminal system monitor via the Lua API.

local sysmon = require("sysmon")

-- 1. Register the CPU Widget
sysmon.register_widget("1. CPU Percent", {
    render = function()
        local usage = sysmon.get_cpu_usage()
        
        local color = "green"
        if usage > 80 then
            color = "red"
        elseif usage > 50 then
            color = "yellow"
        end
        
        -- Use concatenation to guarantee percent sign rendering
        return string.format("%.1f", usage) .. "%", color
    end
})

-- 2. Register the RAM Widget
sysmon.register_widget("2. RAM Usage", {
    render = function()
        -- Convert bytes to gigabytes
        local total_gb = sysmon.get_total_memory() / 1024 / 1024 / 1024
        local used_gb = sysmon.get_used_memory() / 1024 / 1024 / 1024
        local pct = sysmon.get_memory_percent()
        
        local color = "green"
        if pct > 85 then
            color = "red"
        elseif pct > 65 then
            color = "yellow"
        end
        
        -- Use concatenation to cleanly format the closing parenthesis
        return string.format("%.2f GB / %.2f GB (%.1f", used_gb, total_gb, pct) .. "%)", color
    end
})

-- 3. Register the GPU Widget
sysmon.register_widget("3. GPU Usage", {
    render = function()
        local usage = sysmon.get_gpu_usage()
        
        -- If usage is negative, NVML was not initialized or is unavailable
        if usage < 0 then
            return "Not Detected / Unsupported (NVML failed to load)", "gray"
        end
        
        local name = sysmon.get_gpu_name()
        
        -- Query VRAM (convert to megabytes)
        local total_vram_mb = sysmon.get_gpu_memory_total() / 1024 / 1024
        local used_vram_mb = sysmon.get_gpu_memory_used() / 1024 / 1024
        
        local color = "green"
        if usage > 80 then
            color = "red"
        elseif usage > 50 then
            color = "yellow"
        end
        
        -- Format GPU load and VRAM usage using concatenation
        local load_str = string.format("%s | Load: %.1f", name, usage) .. "%"
        local vram_str = string.format(" | VRAM: %.0f MB / %.0f MB", used_vram_mb, total_vram_mb)
        return load_str .. vram_str, color
    end
})
