## packages folder
packages
 - packages.fdb
 - chnobli_ui
  - package.fdb // json_db containing information about the package
  - left
  - right

package.fdb
 - name
 - version_str
 - version // hash
 - signature // signature of the current version
 - current // folder of the current left|right
 - binary // Option<String>

## Todo
maybe add custom error