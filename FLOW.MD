# LensCast Video Processing Architecture

## Objective

The current LensCast implementation streams the Android camera directly into a virtual webcam using scrcpy and v4l2loopback.

Current flow:

```text
Android Phone
      ↓
    scrcpy
      ↓
 v4l2loopback
      ↓
 Zoom / Meet / OBS
```

This architecture works for basic webcam streaming but does not allow real-time video transformations.

The immediate goal is to support the following video transformations:

* Rotate 90°
* Rotate 180°
* Rotate 270°
* Horizontal Flip
* Vertical Flip

---

# Proposed Architecture

```text
Android Phone
      ↓
    scrcpy
      ↓
   GStreamer
      ↓
 v4l2loopback
      ↓
 Zoom / Meet / OBS
```

GStreamer should be introduced between scrcpy and v4l2loopback so video frames can be processed before being exposed as a virtual webcam.

---

# Why GStreamer

GStreamer provides:

* Real-time video processing
* Runtime pipeline updates
* Low latency
* Hardware acceleration support
* Native Linux integration
* Excellent Rust bindings

This makes it suitable for implementing rotation and flip operations without restarting the camera stream.

---

# Required Features

## Rotation

Support:

* 90° Clockwise
* 90° Counter-Clockwise
* 180°
* Reset to Original Orientation

Expected behavior:

* Changes should be applied immediately.
* No stream restart should be required.
* Output should remain compatible with all applications using the virtual webcam.

---

## Flip

Support:

* Horizontal Flip
* Vertical Flip

Expected behavior:

* Changes should be applied immediately.
* Multiple transformations should be allowed.
* Output should remain compatible with all applications using the virtual webcam.

---

# Suggested Pipeline

```text
scrcpy
   ↓
videoflip
   ↓
videoconvert
   ↓
v4l2sink
```

Example operations:

| Feature         | GStreamer Operation               |
| --------------- | --------------------------------- |
| Rotate 90°      | videoflip method=clockwise        |
| Rotate 180°     | videoflip method=rotate-180       |
| Rotate 270°     | videoflip method=counterclockwise |
| Horizontal Flip | videoflip method=horizontal-flip  |
| Vertical Flip   | videoflip method=vertical-flip    |

---

# LensCast Responsibilities

LensCast should:

* Create and manage the GStreamer pipeline
* Apply user-selected transformations
* Update transformations while streaming
* Expose the processed video to v4l2loopback

scrcpy should remain responsible only for:

* Device communication
* Camera capture
* Video transmission

---

# UI Requirements

A new "Video Transformations" section should be available for connected devices.

Controls required:

* Rotation Dropdown

  * Original
  * 90°
  * 180°
  * 270°

* Horizontal Flip Toggle

* Vertical Flip Toggle

Changes should be reflected instantly in the virtual webcam output.

---

# Design Requirement

The implementation must be modular so additional video transformations can be added later without redesigning the pipeline.

The video processing layer should be independent from:

* Device pairing
* Device management
* UI state management
* Virtual webcam management

Only video transformation logic should be handled by the processing pipeline.
