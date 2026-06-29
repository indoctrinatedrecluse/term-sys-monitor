-- config.lua
-- This file configures the terminal system monitor via the Lua API.

local sysmon = require("sysmon")

-- Register the CPU widget
sysmon.register_widget("cpu_percent", {
    render = function()
        -- sysmon.get_cpu_usage() is exposed by Rust
        local usage = sysmon.get_cpu_usage()
        
        -- Determine color based on threshold
        local color = "green"
        if usage > 80 then
            color = "red"
        elseif usage > 50 then
            color = "yellow"
        end
        
        -- Return the display text and its color
        return string.format("CPU Usage: %.1f%%", usage), color
    end
})
