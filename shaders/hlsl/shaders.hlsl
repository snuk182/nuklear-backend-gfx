SamplerState Texture_;
Texture2D<float4> Texture;

cbuffer Locals {
	float4x4 ProjMtx;
};

struct PS_INPUT
{
  float4 pos : SV_POSITION;
  float4 col : COLOR0;
  float2 uv  : TEXCOORD0;
};

PS_INPUT Vertex(float2 pos : Position, float4 col : Color, float2 uv : TexCoord)
{
  PS_INPUT output;
  output.pos = mul(ProjMtx, float4(pos.xy, 0.f, 1.f));
  output.col = col;
  output.uv  = uv;
  return output;
}

float4 Pixel(PS_INPUT input) : SV_Target
{
  return input.col * Texture.Sample(Texture_, input.uv);
}
