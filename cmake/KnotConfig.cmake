# Installed at <prefix>/lib/cmake/Knot; no source-tree or network dependency.
include_guard(GLOBAL)
get_filename_component(_Knot_prefix "${CMAKE_CURRENT_LIST_DIR}/../../.." ABSOLUTE)
include("${CMAKE_CURRENT_LIST_DIR}/KnotTargets.cmake")
unset(_Knot_prefix)
