@echo on
set FXC="C:\Program Files (x86)\Windows Kits\10\bin\x64\fxc.exe" -nologo
%FXC% /T vs_4_0 /E Vertex /Fo vs.fx shaders.hlsl
%FXC% /T ps_4_0 /E Pixel /Fo ps.fx shaders.hlsl
