struct VertexOutput {
  float4 position : SV_POSITION;
};

VertexOutput vs_main(uint vertex_id : SV_VertexID) {
  const float2 positions[3] = {
      float2(-1.0, -1.0),
      float2(-1.0, 3.0),
      float2(3.0, -1.0),
  };
  VertexOutput output;
  output.position = float4(positions[vertex_id], 0.0, 1.0);
  return output;
}

float4 ps_main(VertexOutput input) : SV_TARGET {
  return float4(input.position.xy * 0.0, 0.0, 0.0);
}
