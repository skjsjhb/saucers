cmake_minimum_required(VERSION 3.21)
project(saucer-bindings LANGUAGES CXX VERSION 5.0.0)

# --------------------------------------------------------------------------------------------------------
# Library switches
# --------------------------------------------------------------------------------------------------------

option(saucer_desktop "Enable support for the desktop module" OFF)
option(saucer_pdf "Enable support for the pdf module" OFF)
option(SAUCERS_SHARED_LIB "Build a shared library" ON)

# --------------------------------------------------------------------------------------------------------
# CMake options
# --------------------------------------------------------------------------------------------------------

set(CMAKE_POSITION_INDEPENDENT_CODE ON)

# --------------------------------------------------------------------------------------------------------
# Setup library
# --------------------------------------------------------------------------------------------------------

if (SAUCERS_SHARED_LIB)
    add_library(${PROJECT_NAME} SHARED)
else ()
    add_library(${PROJECT_NAME} STATIC)
endif ()

add_library(saucer::bindings ALIAS ${PROJECT_NAME})

target_compile_features(${PROJECT_NAME} PRIVATE cxx_std_23)
set_target_properties(${PROJECT_NAME} PROPERTIES CXX_STANDARD 23 CXX_EXTENSIONS OFF CXX_STANDARD_REQUIRED ON)

# if (PROJECT_IS_TOP_LEVEL AND NOT MSVC AND NOT CMAKE_CXX_SIMULATE_ID MATCHES "MSVC")
#     target_compile_options(${PROJECT_NAME} PRIVATE -Wall -Wextra -Wpedantic -Werror -pedantic -pedantic-errors -Wfatal-errors)
# endif ()

if (NOT MSVC)
    target_compile_options(${PROJECT_NAME} PRIVATE -Wno-unknown-warning-option -Wno-missing-field-initializers -Wno-cast-function-type)
endif ()

# --------------------------------------------------------------------------------------------------------
# Export header
# --------------------------------------------------------------------------------------------------------

include("cmake/hide.cmake")
include("cmake/export.cmake")

saucer_bindings_hide_symbols(${PROJECT_NAME})
saucer_bindings_export(${PROJECT_NAME} "SAUCER_EXPORT")

# --------------------------------------------------------------------------------------------------------
# Include directories
# --------------------------------------------------------------------------------------------------------

target_include_directories(${PROJECT_NAME} PUBLIC "include")
target_include_directories(${PROJECT_NAME} PRIVATE "include/saucer" "private")

# --------------------------------------------------------------------------------------------------------
# Add Sources
# --------------------------------------------------------------------------------------------------------

target_sources(${PROJECT_NAME} PRIVATE
        "src/app.cpp"
        "src/options.cpp"

        "src/memory.cpp"

        "src/icon.cpp"
        "src/stash.cpp"
        "src/script.cpp"
        "src/scheme.cpp"

        "src/navigation.cpp"
        "src/preferences.cpp"

        "src/window.cpp"
        "src/webview.cpp"
)

# --------------------------------------------------------------------------------------------------------
# Setup Dependencies
# --------------------------------------------------------------------------------------------------------

include("cmake/cpm.cmake")

set(saucer_static ON)
set(CMAKE_WINDOWS_EXPORT_ALL_SYMBOLS ON)
set(saucer_package_all ON)

add_subdirectory(saucer)

target_link_libraries(${PROJECT_NAME} PUBLIC saucer::saucer)

# --------------------------------------------------------------------------------------------------------
# Setup Modules
# --------------------------------------------------------------------------------------------------------

set(MODULES_INSTALL_DIR "${CMAKE_CURRENT_BINARY_DIR}/..")

include("cmake/module.cmake")

if (saucer_desktop)
    add_subdirectory("modules/desktop")
endif ()

if (saucer_pdf)
    add_subdirectory("modules/pdf")
endif ()

install(
        TARGETS ${PROJECT_NAME}
        ARCHIVE DESTINATION "lib"
        LIBRARY DESTINATION "."
        RUNTIME DESTINATION "."
)