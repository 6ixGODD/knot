# Installed at <prefix>/lib/cmake/Knot; no source-tree or network dependency.
# Imported targets are directory-scoped; allow sibling find_package calls.
# KnotTargets.cmake already avoids redefining targets visible in this scope.
get_filename_component(_Knot_prefix "${CMAKE_CURRENT_LIST_DIR}/../../.." ABSOLUTE)
include("${CMAKE_CURRENT_LIST_DIR}/KnotTargets.cmake")
unset(_Knot_prefix)
