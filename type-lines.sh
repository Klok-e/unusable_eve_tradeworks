#!/bin/bash

set -e

# Identifier for the session
session_id="my_session"

# Temporary file to store the input lines
input_file="/tmp/input_lines_${session_id}.txt"
# File to keep track of the current line number
line_counter_file="/tmp/line_counter_${session_id}.txt"

# Check if stdin has new data
if [ -t 0 ]; then
  # No new stdin data; proceed with existing data
  :
else
  # New stdin data detected; reset and use the new data
  cat > "$input_file"  # Overwrite the input file with new stdin data
  echo 0 > "$line_counter_file"  # Reset the line counter
  exit 0
fi

# Read the current line counter
line_counter=$(cat "$line_counter_file")

# Increment the line counter to get the next line
line_counter=$((line_counter + 1))
echo $line_counter > "$line_counter_file"

# Read the specific line from the input file
line=$(sed "${line_counter}q;d" "$input_file")

# If the line is not empty, type it out
if [ ! -z "$line" ]; then
    ydotool key -d 50 29:1 # Keycode for Ctrl key down
    ydotool key -d 50 30:1 30:0 # Keycode for 'a' key
    ydotool key -d 50 29:0 # Keycode for Ctrl key up
    wtype -d 20 "$line"
else
    # If no line is found (end of input), clean up
    echo "End of input reached or no lines left, cleaning up..."
    rm -f "$input_file" "$line_counter_file"
fi