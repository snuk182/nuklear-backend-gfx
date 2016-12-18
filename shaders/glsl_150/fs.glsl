#version 150
precision mediump float;
uniform sampler2D Texture;
in vec2 Frag_UV;
in vec4 Frag_Color;
out vec4 Target0;
void main(){
   Target0 = Frag_Color * texture(Texture, Frag_UV.st);
}
