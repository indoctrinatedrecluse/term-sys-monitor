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

-- Helper to construct a multi-color progress bar table of spans
local function make_colored_bar(percent, width)
    local width = width or 20
    local filled = math.floor((percent / 100) * width)
    local empty = width - filled
    
    local color = "green"
    if percent > 80 then
        color = "red"
    elseif percent > 50 then
        color = "yellow"
    end
    
    return {
        { "[", "cyan" },
        { string.rep("█", filled), color },
        { string.rep("░", empty), "gray" },
        { "]", "cyan" }
    }
end

-- Helper to concatenate two span lists
local function merge_spans(spans1, spans2)
    local result = {}
    for _, s in ipairs(spans1) do
        table.insert(result, s)
    end
    for _, s in ipairs(spans2) do
        table.insert(result, s)
    end
    return result
end

-- 0. Register System Info Widget (Cyan Status Bar)
sysmon.register_widget("0. System Info", {
    render = function()
        local hostname = sysmon.get_hostname()
        local os_name = sysmon.get_os_name()
        local uptime_str = format_uptime(sysmon.get_uptime())
        local cpu_brand = sysmon.get_cpu_brand():gsub("%s+", " ")
        local cpu_freq = sysmon.get_cpu_frequency()
        
        local text = string.format("%s │ %s │ CPU: %s @ %d MHz │ Uptime: %s", hostname, os_name, cpu_brand, cpu_freq, uptime_str)
        return text, "cyan"
    end
})

-- 1. CPU Percent with Sparkline History Graph
local cpu_history = {}
sysmon.register_widget("1. CPU Percent", {
    render = function()
        local usage = sysmon.get_cpu_usage()
        
        -- Store history up to 30 values
        table.insert(cpu_history, usage)
        if #cpu_history > 30 then
            table.remove(cpu_history, 1)
        end
        
        local bar = make_colored_bar(usage, 20)
        local spark = sysmon.create_sparkline(cpu_history)
        
        return merge_spans(bar, {
            { string.format("  %5.1f%%  ", usage), "white" },
            { "Trend: ", "gray" },
            { spark, "magenta" }
        })
    end
})

-- 2. RAM Usage with Multi-color Bar
sysmon.register_widget("2. RAM Usage", {
    render = function()
        local total_gb = sysmon.get_total_memory() / 1024 / 1024 / 1024
        local used_gb = sysmon.get_used_memory() / 1024 / 1024 / 1024
        local pct = sysmon.get_memory_percent()
        
        local bar = make_colored_bar(pct, 20)
        return merge_spans(bar, {
            { string.format("  %5.2f GB / %5.2f GB (%5.1f", used_gb, total_gb, pct) .. "%)", "white" }
        })
    end
})

-- 3. GPU Usage with Multi-color Bar
sysmon.register_widget("3. GPU Usage", {
    render = function()
        local usage = sysmon.get_gpu_usage()
        
        if usage < 0 then
            return "Not Detected / Unsupported (NVML failed to load)", "gray"
        end
        
        local name = sysmon.get_gpu_name()
        local total_vram_mb = sysmon.get_gpu_memory_total() / 1024 / 1024
        local used_vram_mb = sysmon.get_gpu_memory_used() / 1024 / 1024
        
        local bar = make_colored_bar(usage, 20)
        return merge_spans(bar, {
            { string.format("  Load: %5.1f%%  ", usage), "white" },
            { name .. " │ VRAM: " .. string.format("%.0f MB / %.0f MB", used_vram_mb, total_vram_mb), "cyan" }
        })
    end
})

-- 4. Dynamic Disk Registry
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
                    
                    local bar = make_colored_bar(pct, 20)
                    local label = (d.name ~= "") and d.name or "Local Volume"
                    
                    return merge_spans(bar, {
                        { string.format("  %5.1f GB / %5.1f GB (%5.1f", used, total, pct) .. "%)", "white" },
                        { " │ label: " .. label, "cyan" }
                    })
                end
            end
            return "Disconnected", "gray"
        end
    })
end

-- 5. Top 5 CPU Processes (Dynamic widget listing)
for i = 1, 5 do
    local key = string.format("5.%d. Task", i)
    sysmon.register_widget(key, {
        render = function()
            local procs = sysmon.get_processes("cpu", 5)
            local proc = procs[i]
            
            if not proc then
                return " - ", "gray"
            end
            
            local mem_mb = proc.memory / 1024 / 1024
            
            return {
                { string.format("PID: %-6d", proc.pid), "gray" },
                { string.format(" │  %-15.15s", proc.name), "white" },
                { string.format(" │  CPU: %5.1f%%", proc.cpu_usage), (proc.cpu_usage > 50 and "yellow" or (proc.cpu_usage > 80 and "red" or "green")) },
                { string.format(" │  MEM: %6.1f MB", mem_mb), "cyan" }
            }
        end
    })
end
