-- config.lua
-- This file configures the terminal system monitor via the Lua API.

local sysmon = require("sysmon")

-- Utility to format uptime in a human-readable format
local function format_uptime(secs)
    local days = math.floor(secs / 86400)
    local hours = math.floor((secs % 86400) / 3600)
    local mins = math.floor((secs % 3600) / 60)
    local s = secs % 60
    
    if days > 0 then
        return string.format("%dd %dh %dm", days, hours, mins)
    elseif hours > 0 then
        return string.format("%dh %dm", hours, mins)
    else
        return string.format("%dm %ds", mins, s)
    end
end

-- 0. Register System Info Widget (Demonstrating static & diagnostic APIs)
sysmon.register_widget("0. System Info", {
    render = function()
        local hostname = sysmon.get_hostname()
        local os_name = sysmon.get_os_name()
        local uptime_str = format_uptime(sysmon.get_uptime())
        local cpu_brand = sysmon.get_cpu_brand()
        local cpu_freq = sysmon.get_cpu_frequency()
        
        -- Clean up double spaces that some CPU brands report
        cpu_brand = cpu_brand:gsub("%s+", " ")
        
        local text = string.format("%s │ %s │ CPU: %s @ %d MHz │ Uptime: %s", hostname, os_name, cpu_brand, cpu_freq, uptime_str)
        return text, "cyan"
    end
})

-- 1. Register the CPU Widget
sysmon.register_widget("1. CPU Percent", {
    render = function()
        local usage = sysmon.get_cpu_usage()
        local bar = sysmon.create_bar(usage, 20)
        local display_str = string.format("[%s]  %.1f", bar, usage) .. "%"
        
        local color = "green"
        if usage > 80 then
            color = "red"
        elseif usage > 50 then
            color = "yellow"
        end
        
        return display_str, color
    end
})

-- 2. Register the RAM Widget
sysmon.register_widget("2. RAM Usage", {
    render = function()
        local total_gb = sysmon.get_total_memory() / 1024 / 1024 / 1024
        local used_gb = sysmon.get_used_memory() / 1024 / 1024 / 1024
        local pct = sysmon.get_memory_percent()
        
        local bar = sysmon.create_bar(pct, 20)
        local display_str = string.format("[%s]  %.2f GB / %.2f GB (%.1f", bar, used_gb, total_gb, pct) .. "%)"
        
        local color = "green"
        if pct > 85 then
            color = "red"
        elseif pct > 65 then
            color = "yellow"
        end
        
        return display_str, color
    end
})

-- 3. Register the GPU Widget
sysmon.register_widget("3. GPU Usage", {
    render = function()
        local usage = sysmon.get_gpu_usage()
        
        if usage < 0 then
            return "Not Detected / Unsupported (NVML failed to load)", "gray"
        end
        
        local name = sysmon.get_gpu_name()
        local total_vram_mb = sysmon.get_gpu_memory_total() / 1024 / 1024
        local used_vram_mb = sysmon.get_gpu_memory_used() / 1024 / 1024
        
        local bar = sysmon.create_bar(usage, 20)
        local load_str = string.format("[%s]  %s | Load: %.1f", bar, name, usage) .. "%"
        local vram_str = string.format(" | VRAM: %.0f MB / %.0f MB", used_vram_mb, total_vram_mb)
        
        local color = "green"
        if usage > 80 then
            color = "red"
        elseif usage > 50 then
            color = "yellow"
        end
        
        return load_str .. vram_str, color
    end
})

-- 4. Dynamic Disk Registry (Creates separate widgets for all mounted drives)
local disks = sysmon.get_disks()
for i, disk in ipairs(disks) do
    local key = string.format("4.%d. Disk %s", i, disk.mount_point)
    sysmon.register_widget(key, {
        render = function()
            local current_disks = sysmon.get_disks()
            for _, d in ipairs(current_disks) do
                if d.mount_point == disk.mount_point then
                    local total = d.total_space / 1024 / 1024 / 1024
                    local avail = d.available_space / 1024 / 1024 / 1024
                    local used = total - avail
                    local pct = (used / total) * 100
                    
                    local bar = sysmon.create_bar(pct, 20)
                    local color = "green"
                    if pct > 90 then
                        color = "red"
                    elseif pct > 75 then
                        color = "yellow"
                    end
                    
                    local label = (d.name ~= "") and d.name or "Local Volume"
                    return string.format("[%s]  %.1f GB / %.1f GB (%.1f", bar, used, total, pct) .. "%) | label: " .. label, color
                end
            end
            return "Disconnected", "gray"
        end
    })
end
