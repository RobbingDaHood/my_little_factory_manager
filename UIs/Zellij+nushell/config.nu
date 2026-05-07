# Minimal nushell config used only by the bottom command pane.
# Loads the player-command module so commands are available immediately.

$env.config = {
    show_banner: false,
    edit_mode: emacs,
}

# Custom prompt — short and stays on a single line.
def create_left_prompt [] { "mlfm> " }
def create_right_prompt [] { "" }
$env.PROMPT_COMMAND = { || create_left_prompt }
$env.PROMPT_COMMAND_RIGHT = { || create_right_prompt }
$env.PROMPT_INDICATOR = ""
$env.PROMPT_INDICATOR_VI_INSERT = ""
$env.PROMPT_INDICATOR_VI_NORMAL = ""
$env.PROMPT_MULTILINE_INDICATOR = ""

use ./bin/mlfm.nu *

print "My Little Factory Manager — command pane ready."
print "Type `help-mlfm` for the command list, or just `new-game`."
