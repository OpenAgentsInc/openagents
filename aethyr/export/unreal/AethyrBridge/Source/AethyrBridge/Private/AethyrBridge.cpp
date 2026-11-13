#include "CoreMinimal.h"
#include "Modules/ModuleManager.h"
#include "AethyrActors.h"
#include "AethyrEditorChecks.h"
#include "Components/BoxComponent.h"
#include "Components/SceneComponent.h"
#include "Engine/StaticMesh.h"
#include "StaticMeshResources.h"
#include "Materials/MaterialInterface.h"
#include "Materials/Material.h"
#if WITH_EDITOR
#include "AssetCompilingManager.h"
#include "ShaderCompiler.h"
#include "MaterialShared.h"
#include "RHIShaderPlatform.h"
#endif

IMPLEMENT_MODULE(FDefaultModuleImpl, AethyrBridge);

AAethyrCollisionBox::AAethyrCollisionBox()
{
	Box = CreateDefaultSubobject<UBoxComponent>(TEXT("Box"));
	Box->SetCollisionProfileName(TEXT("BlockAll"));
	Box->SetCanEverAffectNavigation(true);
	RootComponent = Box;
}

AAethyrMarker::AAethyrMarker()
{
	RootComponent = CreateDefaultSubobject<USceneComponent>(TEXT("Root"));
}

bool UAethyrEditorChecks::ReadMesh(UStaticMesh* Mesh, TArray<FVector>& Positions, TArray<FColor>& Colors, TArray<int32>& Indices)
{
	Positions.Reset();
	Colors.Reset();
	Indices.Reset();
	if (!Mesh || !Mesh->GetRenderData() || Mesh->GetRenderData()->LODResources.Num() == 0)
	{
		return false;
	}
	const FStaticMeshLODResources& Lod = Mesh->GetRenderData()->LODResources[0];
	const int32 Count = Lod.VertexBuffers.PositionVertexBuffer.GetNumVertices();
	const bool bHasColors = Lod.VertexBuffers.ColorVertexBuffer.GetNumVertices() == (uint32)Count;
	Positions.Reserve(Count);
	Colors.Reserve(Count);
	for (int32 I = 0; I < Count; I++)
	{
		Positions.Add(FVector(Lod.VertexBuffers.PositionVertexBuffer.VertexPosition(I)));
		Colors.Add(bHasColors ? Lod.VertexBuffers.ColorVertexBuffer.VertexColor(I) : FColor(0, 0, 0, 0));
	}
	TArray<uint32> Raw;
	Lod.IndexBuffer.GetCopy(Raw);
	Indices.Reserve(Raw.Num());
	for (uint32 V : Raw)
	{
		Indices.Add((int32)V);
	}
	return bHasColors;
}

void UAethyrEditorChecks::FinishCompiling()
{
#if WITH_EDITOR
	FAssetCompilingManager::Get().FinishAllCompilation();
	if (GShaderCompilingManager)
	{
		GShaderCompilingManager->FinishAllCompilation();
	}
#endif
}

TArray<FString> UAethyrEditorChecks::MaterialErrors(UMaterialInterface* Material)
{
	TArray<FString> Errors;
#if WITH_EDITOR
	if (!Material)
	{
		Errors.Add(TEXT("no material"));
		return Errors;
	}
	if (const FMaterialResource* Resource = Material->GetMaterialResource(GMaxRHIShaderPlatform))
	{
		Errors = Resource->GetCompileErrors();
	}
	else
	{
		Errors.Add(TEXT("no material resource for the running shader platform"));
	}
#endif
	return Errors;
}
