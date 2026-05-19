# Odek - 3D Engine

-[Web Version](https://barbasoyun.github.io/3DEngine/)<br>
-Odek is a 3D Engine written in Rust and using egui for rendering<br>
-It as both a desktop/native and web version<br>
-WIP<br>
-Using [This Tsoding video](https://www.youtube.com/watch?v=qjWkNZ0SXfo&list=WL&index=9&t=17s) as reference<br>

## Controls

-Move Camera = WASD (ZQSD if AZERTY is enabled)<br>
-Rotate Camera = Right Click<br>

## Features

-Render 3D Models Wireframe<br>
-Transform 3D Model<br>
-Move + Rotate Camera<br>
-Import OBJ File<br>
-Right-Handed 3D (Z forward = -Z, Camera start at 180° Y Rotation) Similar to : Blender, Godot, OpenGl, Vulkan<br>
-GPU computing : the vertices are multiplied by the MVP matrix using the GPU and handed back to egui's Render Pipeline<br>
(it's a weird setup that allowed me to discover GPU Computing)<br>

## Learnings

-Applied Maths : Linear Algebra, Intercept Theorem, Vector Operations (Dot Product, Cross Product), Matrix Multiplication<br>
-Camera System<br>
-Rendering Steps<br>
-Graphics Programming in General<br>
-Disovered GPU Computing<br>

-A lot of dead code / comments but I want to keep it as exemple (Old Engine)<br>

## To Go Further

-Optimisations : Triangulate Faces, ...<br>
-Render Faces<br>
-Calculate Face Normals<br>
-Lighting<br>
-Textures<br>

## Progress

3D Models used for the gifs:<br>
-[Penger](https://github.com/Max-Kawula/penger-obj)<br>
-[Jurassic Park 3 Spinosaurus](https://sketchfab.com/3d-models/jurassic-park-3-spinosaurus-0bcc1f6d3f1e491b9164f3e21fec8b19)<br>

![Spinning Cube](assets/3DEngine.gif)
![Penger my Beloved](assets/Penger.gif)
![Spinosaurus](assets/SPINosaurus.gif)
![Transformations](assets/Transformations.gif)
